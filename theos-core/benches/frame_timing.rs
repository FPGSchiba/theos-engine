//! Benchmarks for frame-dispatch overhead in TheosKernel.
//!
//! These benchmarks isolate the cost of `step(dt)` on N mounted modules,
//! independently of the sleep/spin timing in the variable-step loop.
//!
//! Run with:
//!   cargo bench -p theos-core --bench frame_timing

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use seq_macro::seq;
use std::any::TypeId;
use std::hint::black_box;
use theos_core::TheosKernel;
use theos_shared::{EngineModule, KernelContext};

// ---------------------------------------------------------------------------
// Per-TAG no-op module — tick does nothing, giving pure dispatch overhead.
//
// `DependencyResolver::sort` deduplicates modules by `module_type_id()` (its
// internal `HashMap<TypeId, ModuleInfo>`); registering N modules that share a
// `TypeId` collapses them into a single resolver node and causes
// `kernel.init()` to fail with `CircularDependency`. We therefore parameterise
// `NoOpModule` over a `const TAG: usize` so each value of `TAG` is a distinct
// monomorphisation with its own `TypeId`.
//
// `register_module` also rejects duplicate names, so each `TAG` resolves to a
// unique `&'static str` via the static `MODULE_NAMES` table below.
// ---------------------------------------------------------------------------

/// Maximum modules a single bench iteration registers. Matches the largest
/// value in `bench_tick_dispatch`'s input slice.
const MAX_MODULES: usize = 50;

struct NoOpModule<const TAG: usize>;

impl<const TAG: usize> EngineModule for NoOpModule<TAG> {
    fn name(&self) -> &'static str {
        MODULE_NAMES[TAG]
    }

    fn init(
        &mut self,
        _ctx: &mut KernelContext,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    fn module_type_id(&self) -> TypeId {
        TypeId::of::<NoOpModule<TAG>>()
    }
}

// Static array of 50 unique `&'static str` names, built at compile time via
// `seq!` so every entry is a distinct literal (`NoOpModule_0` … `NoOpModule_49`).
seq!(N in 0..50 {
    static MODULE_NAMES: [&str; MAX_MODULES] = [
        #(
            concat!("NoOpModule_", stringify!(N)),
        )*
    ];
});

/// Registers `n` distinct `NoOpModule<TAG>` instances on `kernel`. Panics if
/// `n > MAX_MODULES` — extend the seq! upper bound and `MAX_MODULES` if a
/// larger fan-out is needed.
fn register_n_modules(kernel: &mut TheosKernel, n: usize) {
    assert!(
        n <= MAX_MODULES,
        "register_n_modules: requested {n} modules, max is {MAX_MODULES}"
    );
    seq!(N in 0..50 {
        #(
            if n > N {
                kernel
                    .register_module(Box::new(NoOpModule::<N>))
                    .expect("register_module failed");
            }
        )*
    });
}

// ---------------------------------------------------------------------------
// Bench: step() dispatch with N no-op modules
// ---------------------------------------------------------------------------

fn bench_tick_dispatch(c: &mut Criterion) {
    let mut group = c.benchmark_group("tick_dispatch");

    for &n_modules in &[1usize, 5, 10, 20, 50] {
        group.bench_with_input(
            BenchmarkId::new("modules", n_modules),
            &n_modules,
            |b, &n| {
                // Setup outside the timed region: build kernel, register N
                // unique-typed no-op modules, run init() so step() is callable.
                let mut kernel = TheosKernel::default();
                register_n_modules(&mut kernel, n);
                kernel.init().expect("kernel init failed");

                // Timed region: one step dispatch per iteration.
                b.iter(|| {
                    kernel.step(black_box(0.016_667)).expect("step failed");
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_tick_dispatch);
criterion_main!(benches);
