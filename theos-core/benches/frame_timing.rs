//! Benchmarks for frame-dispatch overhead in TheosKernel.
//!
//! These benchmarks isolate the cost of calling `tick()` on N mounted modules,
//! independently of the sleep/spin timing in the fixed-step loop.
//!
//! Run with:
//!   cargo bench -p theos-core --bench frame_timing

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use std::any::TypeId;
use std::hint::black_box;
use theos_core::TheosKernel;
use theos_shared::{EngineModule, KernelContext};

// ---------------------------------------------------------------------------
// No-op module — tick does nothing, giving pure dispatch overhead.
// ---------------------------------------------------------------------------

struct NoOpModule {
    #[allow(dead_code)]
    id: usize,
}

impl EngineModule for NoOpModule {
    fn name(&self) -> &'static str {
        "NoOpModule"
    }

    fn init(
        &mut self,
        _ctx: &mut KernelContext,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    fn module_type_id(&self) -> TypeId {
        // Each instance needs a unique TypeId so they don't collide in the
        // dependency resolver. We side-step this by never calling run() in
        // these benchmarks — we drive tick_once() directly. The TypeId
        // returned here is only used if resolve_init_order() is called.
        TypeId::of::<NoOpModule>()
    }
}

// ---------------------------------------------------------------------------
// Bench: tick dispatch with N no-op modules
// ---------------------------------------------------------------------------

fn bench_tick_dispatch(c: &mut Criterion) {
    let mut group = c.benchmark_group("tick_dispatch");

    for &n_modules in &[1usize, 5, 10, 20, 50] {
        group.bench_with_input(
            BenchmarkId::new("modules", n_modules),
            &n_modules,
            |b, &n| {
                // Setup: build a kernel with N no-op modules.
                // This is outside the timed region.
                let mut kernel = TheosKernel::default();
                for id in 0..n {
                    kernel.register_module(Box::new(NoOpModule { id })).unwrap();
                }

                // Timed region: one tick dispatch per iteration.
                b.iter(|| {
                    kernel.step(black_box(0.016_667));
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_tick_dispatch);
criterion_main!(benches);
