use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;

use theos_core::event::{EventBus, GameEvent};

const BATCH: usize = 1024;

fn bench_send_then_drain_typed(c: &mut Criterion) {
    c.bench_function("send_then_drain_typed_batch_1024", |b| {
        b.iter(|| {
            let mut bus = EventBus::default();
            bus.register::<GameEvent>();

            let sender = bus.sender_for::<GameEvent>().expect("registered");

            for _ in 0..BATCH {
                sender.send(black_box(GameEvent::Dummy)).unwrap();
            }

            let drained = bus.drain::<GameEvent>().unwrap();
            black_box(drained);
        });
    });
}

fn bench_send_then_drain_boxed_baseline(c: &mut Criterion) {
    use crossbeam::channel;

    c.bench_function("send_then_drain_boxed_batch_1024", |b| {
        b.iter(|| {
            let (s, r) = channel::unbounded::<Box<GameEvent>>();

            for _ in 0..BATCH {
                let boxed = Box::new(black_box(GameEvent::Dummy));
                s.send(boxed).unwrap();
            }

            let mut got = Vec::with_capacity(BATCH);
            loop {
                match r.try_recv() {
                    Ok(bx) => got.push(bx),
                    Err(crossbeam::channel::TryRecvError::Empty) => break,
                    Err(crossbeam::channel::TryRecvError::Disconnected) => break,
                }
            }

            black_box(got);
        });
    });
}

criterion_group!(
    benches,
    bench_send_then_drain_typed,
    bench_send_then_drain_boxed_baseline
);
criterion_main!(benches);
