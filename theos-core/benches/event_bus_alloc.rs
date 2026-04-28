use alloc_count::alloc_count;
use theos_core::event::{EventBus, GameEvent};

const BATCH: usize = 1024;

#[test]
fn allocation_check_typed_event_bus() {
    let mut bus = EventBus::new();
    bus.register::<GameEvent>().unwrap();

    let (stats, ()) = alloc_count!({
        let sender = bus.sender_for::<GameEvent>().expect("registered");

        for _ in 0..BATCH {
            sender.send(GameEvent::Dummy).unwrap();
        }

        let drained = bus.drain::<GameEvent>().unwrap();
        drop(drained);
    });

    println!("typed alloc stats: {:?}", stats);
}

#[test]
fn allocation_check_boxed_baseline() {
    use crossbeam::channel;

    let (stats, ()) = alloc_count!({
        let (s, r) = channel::unbounded::<Box<GameEvent>>();

        for _ in 0..BATCH {
            let boxed = Box::new(GameEvent::Dummy);
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

        drop(got);
    });

    println!("boxed alloc stats: {:?}", stats);
}