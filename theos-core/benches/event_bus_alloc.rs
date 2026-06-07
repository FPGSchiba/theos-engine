#![allow(clippy::disallowed_macros)]

use alloc_count::alloc_count;
use theos_core::event::{BroadcastBusExt, BroadcastEventBus, GameEvent};

const BATCH: usize = 1024;

#[test]
fn allocation_check_typed_event_bus() {
    let bus = BroadcastEventBus::default();
    bus.register::<GameEvent>(Some(BATCH * 2)).unwrap();
    let mut receiver = bus.receiver_for::<GameEvent>().expect("registered");

    let (stats, ()) = alloc_count!({
        let sender = bus.sender_for::<GameEvent>().expect("registered");

        for _ in 0..BATCH {
            sender.send(GameEvent::Dummy).unwrap();
        }

        let mut got = Vec::with_capacity(BATCH);
        while let Ok(ev) = receiver.try_recv() {
            got.push(ev);
        }
        drop(got);
    });

    println!("typed alloc stats: {:?}", stats);
}

#[test]
fn allocation_check_boxed_baseline() {
    use tokio::sync::broadcast;

    let (s, mut r) = broadcast::channel::<Box<GameEvent>>(BATCH * 2);

    let (stats, ()) = alloc_count!({
        for _ in 0..BATCH {
            let boxed = Box::new(GameEvent::Dummy);
            s.send(boxed).unwrap();
        }

        let mut got = Vec::with_capacity(BATCH);
        while let Ok(bx) = r.try_recv() {
            got.push(bx);
        }
        drop(got);
    });

    println!("boxed alloc stats: {:?}", stats);
}
