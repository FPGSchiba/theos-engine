use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;

use theos_core::event::{BroadcastBusExt, BroadcastEventBus, GameEvent};

const BATCH: usize = 1024;

fn bench_send_then_recv_typed(c: &mut Criterion) {
    c.bench_function("send_then_recv_typed_batch_1024", |b| {
        b.iter(|| {
            let bus = BroadcastEventBus::default();
            // Capacity sized for the full batch so no Lagged in steady-state.
            bus.register::<GameEvent>(Some(BATCH * 2)).unwrap();
            let mut receiver = bus.receiver_for::<GameEvent>().expect("registered");
            let sender = bus.sender_for::<GameEvent>().expect("registered");

            for _ in 0..BATCH {
                sender.send(black_box(GameEvent::Dummy)).unwrap();
            }

            let mut got = Vec::with_capacity(BATCH);
            while let Ok(ev) = receiver.try_recv() {
                got.push(ev);
            }
            black_box(got);
        });
    });
}

fn bench_send_then_recv_boxed_baseline(c: &mut Criterion) {
    use tokio::sync::broadcast;

    c.bench_function("send_then_recv_boxed_batch_1024", |b| {
        b.iter(|| {
            let (s, mut r) = broadcast::channel::<Box<GameEvent>>(BATCH * 2);

            for _ in 0..BATCH {
                let boxed = Box::new(black_box(GameEvent::Dummy));
                s.send(boxed).unwrap();
            }

            let mut got = Vec::with_capacity(BATCH);
            while let Ok(bx) = r.try_recv() {
                got.push(bx);
            }
            black_box(got);
        });
    });
}

criterion_group!(
    benches,
    bench_send_then_recv_typed,
    bench_send_then_recv_boxed_baseline
);
criterion_main!(benches);
