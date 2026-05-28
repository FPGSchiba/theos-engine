//! Integration tests for the TheosKernel lifecycle.
//!
//! These tests exercise the full kernel: module init, fixed-step tick loop,
//! shutdown signalling, and module shutdown — all running end-to-end.

#![allow(clippy::disallowed_macros)]

use crossbeam::channel::Sender;
use std::any::TypeId;
use std::marker::PhantomData;
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc, Mutex,
};
use std::time::Instant;
use theos_core::TheosKernel;
use theos_shared::{EngineModule, KernelContext};

// ---------------------------------------------------------------------------
// Tag types — give two CountingModule instances distinct TypeIds.
// ---------------------------------------------------------------------------

struct TagOne;
struct TagTwo;

// ---------------------------------------------------------------------------
// Test module: CountingModule<Tag>
//
// Counts how many times init(), tick(), and shutdown() were called.
// Fires the kernel shutdown sender after `stop_after` ticks.
// ---------------------------------------------------------------------------

struct CountingModule<Tag> {
    name: &'static str,
    stop_after: u32,
    shutdown_tx: Arc<Sender<()>>,
    tick_count: Arc<AtomicU32>,
    init_count: Arc<AtomicU32>,
    shutdown_count: Arc<AtomicU32>,
    _tag: PhantomData<Tag>,
}

impl<Tag: Send + Sync + 'static> EngineModule for CountingModule<Tag> {
    fn name(&self) -> &'static str {
        self.name
    }

    fn init(
        &mut self,
        _ctx: &mut KernelContext,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.init_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn tick(&mut self, _dt: f32) {
        self.tick_count.fetch_add(1, Ordering::SeqCst);
        if self.tick_count.load(Ordering::SeqCst) >= self.stop_after {
            self.shutdown_tx.send(()).ok();
        }
    }

    fn shutdown(&mut self) {
        self.shutdown_count.fetch_add(1, Ordering::SeqCst);
    }

    fn module_type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Constructs a CountingModule and returns it alongside its observable counters.
/// Returns `(module, tick_count, init_count, shutdown_count)`.
fn make_counting_module<Tag: Send + Sync + 'static>(
    name: &'static str,
    stop_after: u32,
    shutdown_tx: Arc<Sender<()>>,
) -> (
    CountingModule<Tag>,
    Arc<AtomicU32>,
    Arc<AtomicU32>,
    Arc<AtomicU32>,
) {
    let tick_count = Arc::new(AtomicU32::new(0));
    let init_count = Arc::new(AtomicU32::new(0));
    let shutdown_count = Arc::new(AtomicU32::new(0));

    let module = CountingModule {
        name,
        stop_after,
        shutdown_tx,
        tick_count: tick_count.clone(),
        init_count: init_count.clone(),
        shutdown_count: shutdown_count.clone(),
        _tag: PhantomData,
    };

    (module, tick_count, init_count, shutdown_count)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn kernel_runs_init_tick_shutdown_lifecycle() {
    let mut kernel = TheosKernel::default();
    let shutdown_tx = Arc::new(kernel.shutdown_handle());

    let (mod_one, tick_one, init_one, shutdown_one) =
        make_counting_module::<TagOne>("CountingModuleOne", 10, shutdown_tx.clone());
    let (mod_two, tick_two, init_two, shutdown_two) =
        make_counting_module::<TagTwo>("CountingModuleTwo", 1000, shutdown_tx.clone());

    kernel.register_module(Box::new(mod_one)).unwrap();
    kernel.register_module(Box::new(mod_two)).unwrap();
    kernel.run().unwrap();

    assert_eq!(
        init_one.load(Ordering::SeqCst),
        1,
        "module one init should be called exactly once"
    );
    assert_eq!(
        init_two.load(Ordering::SeqCst),
        1,
        "module two init should be called exactly once"
    );

    assert!(
        tick_one.load(Ordering::SeqCst) >= 10,
        "module one should tick at least 10 times"
    );
    assert!(
        tick_two.load(Ordering::SeqCst) >= 10,
        "module two should tick at least 10 times"
    );

    assert_eq!(
        shutdown_one.load(Ordering::SeqCst),
        1,
        "module one shutdown should be called exactly once"
    );
    assert_eq!(
        shutdown_two.load(Ordering::SeqCst),
        1,
        "module two shutdown should be called exactly once"
    );
}

#[test]
fn shutdown_before_run_exits_immediately() {
    let mut kernel = TheosKernel::default();
    let shutdown_tx = Arc::new(kernel.shutdown_handle());
    shutdown_tx.send(()).unwrap();
    assert!(kernel.run().is_ok());
}

// ---------------------------------------------------------------------------
// Timing accuracy test
// ---------------------------------------------------------------------------

struct TimingModule {
    timestamps: Arc<Mutex<Vec<Instant>>>,
    shutdown_tx: Arc<Sender<()>>,
    stop_after: u32,
    count: Arc<AtomicU32>,
}

impl EngineModule for TimingModule {
    fn name(&self) -> &'static str {
        "TimingModule"
    }

    fn init(
        &mut self,
        _ctx: &mut KernelContext,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    fn tick(&mut self, _dt: f32) {
        self.timestamps.lock().unwrap().push(Instant::now());
        if self.count.fetch_add(1, Ordering::SeqCst) + 1 >= self.stop_after {
            self.shutdown_tx.send(()).ok();
        }
    }

    fn module_type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }
}

/// Runs the fixed-step loop for 300 frames and prints timing statistics including
/// a 95% confidence interval for the mean frame time.
///
/// No threshold is asserted — run with:
///   cargo test -p theos-core --release --test kernel_integration reports_frame_timing_accuracy -- --nocapture
#[test]
fn reports_frame_timing_accuracy() {
    const FRAMES: u32 = 60 * 5; // 5 seconds of tests

    let mut kernel = TheosKernel::default();
    let shutdown_tx = Arc::new(kernel.shutdown_handle());
    let timestamps = Arc::new(Mutex::new(Vec::with_capacity(FRAMES as usize)));
    let count = Arc::new(AtomicU32::new(0));

    kernel
        .register_module(Box::new(TimingModule {
            timestamps: timestamps.clone(),
            shutdown_tx,
            stop_after: FRAMES,
            count,
        }))
        .unwrap();

    kernel.run().unwrap();

    let ts = timestamps.lock().unwrap();
    assert_eq!(ts.len() as u32, FRAMES, "expected exactly {FRAMES} ticks");

    let deltas_ms: Vec<f64> = ts
        .windows(2)
        .map(|w| w[1].duration_since(w[0]).as_secs_f64() * 1000.0)
        .collect();

    let n = deltas_ms.len() as f64;
    let mean_ms = deltas_ms.iter().sum::<f64>() / n;
    // Bessel-corrected sample variance for an unbiased stddev estimate.
    let variance = deltas_ms
        .iter()
        .map(|&d| (d - mean_ms).powi(2))
        .sum::<f64>()
        / (n - 1.0);
    let stddev_ms = variance.sqrt();
    let stderr_ms = stddev_ms / n.sqrt();
    let ci95_ms = 1.96 * stderr_ms;
    let max_ms = deltas_ms.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let min_ms = deltas_ms.iter().cloned().fold(f64::INFINITY, f64::min);
    let target_ms = 1000.0 / 60.0;

    println!("=== Frame Timing ({FRAMES} frames, 60 Hz target = {target_ms:.4} ms) ===");
    println!(
        "  Mean        : {mean_ms:.4} ms  (error {:.4} ms)",
        (mean_ms - target_ms).abs()
    );
    println!(
        "  95% CI      : ± {ci95_ms:.4} ms  [{:.4}, {:.4}]",
        mean_ms - ci95_ms,
        mean_ms + ci95_ms
    );
    println!("  Std dev     : {stddev_ms:.4} ms");
    println!("  Min / Max   : {min_ms:.4} ms / {max_ms:.4} ms");
    println!("  Max jitter  : {:.4} ms", max_ms - min_ms);
}

/// Runs N independent kernel instances, each for FRAMES_PER_RUN frames.
///
/// Reports per-run stats and a 95% CI across run-means (using the t-distribution
/// for small N). The per-run max jitter column shows whether spikes are
/// consistent across runs or isolated flukes.
///
/// Run with:
///   cargo test -p theos-core --release --test kernel_integration reports_frame_timing_multirun -- --nocapture
#[test]
fn reports_frame_timing_multirun() {
    const RUNS: usize = 10;
    const FRAMES_PER_RUN: u32 = 60 * 3; // 3 seconds per run (30 seconds in total)
                                        // t-critical values for 95% CI, two-tailed, by degrees of freedom (df = RUNS - 1).
                                        // Lookup table covers df 1–20; beyond 30 the normal approximation (1.96) is fine.
    const T_CRIT_95: [f64; 20] = [
        12.706, 4.303, 3.182, 2.776, 2.571, // df 1–5
        2.447, 2.365, 2.306, 2.262, 2.228, // df 6–10
        2.201, 2.179, 2.160, 2.145, 2.131, // df 11–15
        2.120, 2.110, 2.101, 2.093, 2.086, // df 16–20
    ];
    let t_crit = T_CRIT_95[(RUNS - 1).min(19)];

    let target_ms = 1000.0 / 60.0;
    let mut run_means = Vec::with_capacity(RUNS);
    let mut run_max_jitters = Vec::with_capacity(RUNS);

    println!("=== Multi-Run Frame Timing ({RUNS} × {FRAMES_PER_RUN} frames, target {target_ms:.4} ms) ===");
    println!(
        "  {:>4}  {:>10}  {:>10}  {:>10}  {:>10}",
        "run", "mean (ms)", "error (ms)", "stddev", "max jitter"
    );

    for run in 0..RUNS {
        let mut kernel = TheosKernel::default();
        let shutdown_tx = Arc::new(kernel.shutdown_handle());
        let timestamps = Arc::new(Mutex::new(Vec::with_capacity(FRAMES_PER_RUN as usize)));
        let count = Arc::new(AtomicU32::new(0));

        kernel
            .register_module(Box::new(TimingModule {
                timestamps: timestamps.clone(),
                shutdown_tx,
                stop_after: FRAMES_PER_RUN,
                count,
            }))
            .unwrap();
        kernel.run().unwrap();

        let ts = timestamps.lock().unwrap();
        let deltas_ms: Vec<f64> = ts
            .windows(2)
            .map(|w| w[1].duration_since(w[0]).as_secs_f64() * 1000.0)
            .collect();

        let n = deltas_ms.len() as f64;
        let mean = deltas_ms.iter().sum::<f64>() / n;
        let variance = deltas_ms.iter().map(|&d| (d - mean).powi(2)).sum::<f64>() / (n - 1.0);
        let stddev = variance.sqrt();
        let max = deltas_ms.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let min = deltas_ms.iter().cloned().fold(f64::INFINITY, f64::min);
        let jitter = max - min;

        println!(
            "  {:>4}  {:>10.4}  {:>10.4}  {:>10.4}  {:>10.4}",
            run + 1,
            mean,
            (mean - target_ms).abs(),
            stddev,
            jitter
        );

        run_means.push(mean);
        run_max_jitters.push(jitter);
    }

    // CI across run-means using t-distribution.
    let n = RUNS as f64;
    let grand_mean = run_means.iter().sum::<f64>() / n;
    let var_of_means = run_means
        .iter()
        .map(|&m| (m - grand_mean).powi(2))
        .sum::<f64>()
        / (n - 1.0);
    let stderr = var_of_means.sqrt() / n.sqrt();
    let ci = t_crit * stderr;
    let worst_jitter = run_max_jitters
        .iter()
        .cloned()
        .fold(f64::NEG_INFINITY, f64::max);
    let mean_jitter = run_max_jitters.iter().sum::<f64>() / n;

    println!("---");
    println!(
        "  Grand mean  : {grand_mean:.4} ms  (error {:.4} ms)",
        (grand_mean - target_ms).abs()
    );
    println!(
        "  95% CI      : ± {ci:.4} ms  [{:.4}, {:.4}]",
        grand_mean - ci,
        grand_mean + ci
    );
    println!("  Worst spike : {worst_jitter:.4} ms  (mean {mean_jitter:.4} ms across runs)");
}
