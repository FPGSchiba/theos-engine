//! A dedicated thread that owns an [`Npc`], so inference never runs on the
//! frame loop.
//!
//! The frame loop calls [`NpcWorker::submit`] and later [`NpcWorker::poll`];
//! neither blocks on the model. The [`Npc`] itself is built on the worker
//! thread by a factory closure, because backend contexts are not required to
//! be `Send`.

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;

use crate::driver::{Npc, NpcError, NpcRequest, NpcResponse};

/// Identifies one submitted request; returned by [`NpcWorker::submit`] and
/// handed back alongside the matching response.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Ticket(u64);

impl Ticket {
    /// The ticket's sequence number; the first submitted request gets `0`.
    pub fn index(self) -> u64 {
        self.0
    }
}

/// The worker thread has ended, so no further requests can be served.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("the NPC worker thread has stopped")]
pub struct WorkerGone;

/// One finished request: its ticket and the outcome.
pub type Completed = (Ticket, Result<NpcResponse, NpcError>);

/// Owns a worker thread running an [`Npc`].
///
/// Requests are served one at a time, in submit order. Dropping the worker
/// closes the request channel and joins the thread; a request already running
/// finishes first.
pub struct NpcWorker {
    requests: Option<Sender<(Ticket, NpcRequest)>>,
    responses: Receiver<Completed>,
    handle: Option<JoinHandle<()>>,
    next_ticket: u64,
}

impl NpcWorker {
    /// Starts the worker thread, runs `factory` on it, and waits until it
    /// reports ready.
    ///
    /// # Errors
    ///
    /// Returns the factory's error if it fails, or [`NpcError::WorkerLost`]
    /// if the thread ended (for example by panicking) before reporting.
    ///
    /// # Panics
    ///
    /// Panics if the operating system cannot create a thread.
    pub fn spawn<F>(factory: F) -> Result<NpcWorker, NpcError>
    where
        F: FnOnce() -> Result<Npc, NpcError> + Send + 'static,
    {
        let (request_tx, request_rx) = mpsc::channel::<(Ticket, NpcRequest)>();
        let (response_tx, response_rx) = mpsc::channel::<Completed>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<(), NpcError>>();

        let handle = std::thread::spawn(move || {
            let mut npc = match factory() {
                Ok(npc) => {
                    if ready_tx.send(Ok(())).is_err() {
                        return;
                    }
                    npc
                }
                Err(e) => {
                    // The spawner may already be gone; nothing to do then.
                    let _ = ready_tx.send(Err(e));
                    return;
                }
            };
            while let Ok((ticket, request)) = request_rx.recv() {
                let outcome = npc.answer(&request);
                if response_tx.send((ticket, outcome)).is_err() {
                    return;
                }
            }
        });

        match ready_rx.recv() {
            Ok(Ok(())) => Ok(NpcWorker {
                requests: Some(request_tx),
                responses: response_rx,
                handle: Some(handle),
                next_ticket: 0,
            }),
            Ok(Err(e)) => {
                let _ = handle.join();
                Err(e)
            }
            Err(_) => {
                let _ = handle.join();
                Err(NpcError::WorkerLost)
            }
        }
    }

    /// Queues `request` and returns immediately with its [`Ticket`].
    ///
    /// # Errors
    ///
    /// [`WorkerGone`] if the worker thread has stopped.
    pub fn submit(&mut self, request: NpcRequest) -> Result<Ticket, WorkerGone> {
        let ticket = Ticket(self.next_ticket);
        self.requests
            .as_ref()
            .ok_or(WorkerGone)?
            .send((ticket, request))
            .map_err(|_| WorkerGone)?;
        self.next_ticket += 1;
        Ok(ticket)
    }

    /// Returns a finished request if one is ready, without blocking.
    pub fn poll(&self) -> Option<Completed> {
        self.responses.try_recv().ok()
    }

    /// Blocks until a request finishes; `None` when the worker is gone.
    pub fn wait(&self) -> Option<Completed> {
        self.responses.recv().ok()
    }
}

impl Drop for NpcWorker {
    fn drop(&mut self) {
        // Closing the request channel ends the worker's receive loop.
        self.requests = None;
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{Phase, Piece, Thinking};
    use crate::context::ContextOptions;
    use crate::driver::NpcParts;
    use crate::lore::{RecordId, Speaker};
    use crate::retrieval::LoreScope;
    use crate::test_support::mini;
    use crate::testkit::{FakeClock, FakeEmbedder, FakeProbe, Script, ScriptedBackend};
    use std::time::{Duration, Instant};

    const GOOD: &str = r#"{"sources":["B-1"],"reply":"Aye."}"#;

    fn parts(scripts: usize, speakers: Vec<Speaker>) -> NpcParts {
        let clock = FakeClock::new();
        let backend = ScriptedBackend::new(clock.clone());
        for _ in 0..scripts {
            backend.push(Script {
                prompt_tokens: 1,
                prefill: Duration::from_millis(1),
                per_token: Duration::from_millis(1),
                pieces: GOOD
                    .chars()
                    .map(|c| Piece {
                        text: c.to_string(),
                        phase: Phase::Answer,
                    })
                    .collect(),
            });
        }
        let (corpus, _) = mini();
        NpcParts {
            backend: Box::new(backend),
            embedder: Box::new(FakeEmbedder::default()),
            corpus,
            speakers,
            options: ContextOptions::default(),
            clock: Box::new(clock),
            probe: Box::new(FakeProbe::new(vec![])),
        }
    }

    fn worker(scripts: usize) -> NpcWorker {
        NpcWorker::spawn(move || {
            let (_, speaker) = mini();
            Npc::new(parts(scripts, vec![speaker]))
        })
        .expect("worker spawns")
    }

    fn request() -> NpcRequest {
        NpcRequest {
            speaker: RecordId::new("n"),
            question: "who?".into(),
            scope: LoreScope::AllVisible,
            thinking: Thinking::On,
            deadline: Duration::from_secs(3),
        }
    }

    #[test]
    fn spawn_runs_factory_on_worker_thread() {
        let (tx, rx) = mpsc::channel();
        let _worker = NpcWorker::spawn(move || {
            tx.send(std::thread::current().id()).unwrap();
            let (_, speaker) = mini();
            Npc::new(parts(0, vec![speaker]))
        })
        .unwrap();
        assert_ne!(rx.recv().unwrap(), std::thread::current().id());
    }

    #[test]
    fn spawn_returns_factory_error() {
        let failure = NpcError::UnknownSpeaker(RecordId::new("ghost"));
        let expected = failure.clone();
        let result = NpcWorker::spawn(move || Err(failure));
        assert_eq!(result.err(), Some(expected));
    }

    #[test]
    fn spawn_reports_worker_lost_when_factory_panics() {
        let result = NpcWorker::spawn(|| panic!("factory blew up"));
        assert_eq!(result.err(), Some(NpcError::WorkerLost));
    }

    #[test]
    fn submit_then_wait_returns_response_with_same_ticket() {
        let mut w = worker(1);
        let ticket = w.submit(request()).unwrap();
        let (got, outcome) = w.wait().unwrap();
        assert_eq!(got, ticket);
        assert_eq!(outcome.unwrap().reply.unwrap().text, "Aye.");
    }

    #[test]
    fn responses_come_back_in_submit_order() {
        let mut w = worker(3);
        let tickets: Vec<Ticket> = (0..3).map(|_| w.submit(request()).unwrap()).collect();
        assert_eq!(
            tickets.iter().map(|t| t.index()).collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        for expected in tickets {
            let (got, outcome) = w.wait().unwrap();
            assert_eq!(got, expected);
            assert!(outcome.is_ok());
        }
    }

    #[test]
    fn request_failure_comes_back_as_error_for_its_ticket() {
        let mut w = worker(1);
        let mut bad = request();
        bad.speaker = RecordId::new("ghost");
        let ticket = w.submit(bad).unwrap();
        let (got, outcome) = w.wait().unwrap();
        assert_eq!(got, ticket);
        assert_eq!(
            outcome.err(),
            Some(NpcError::UnknownSpeaker(RecordId::new("ghost")))
        );
    }

    #[test]
    fn poll_returns_none_when_nothing_submitted() {
        let w = worker(0);
        assert!(w.poll().is_none());
    }

    #[test]
    fn poll_eventually_returns_the_response() {
        let mut w = worker(1);
        let ticket = w.submit(request()).unwrap();
        let start = Instant::now();
        let found = loop {
            if let Some(done) = w.poll() {
                break done;
            }
            assert!(
                start.elapsed() < Duration::from_secs(2),
                "no response in 2s"
            );
            std::thread::sleep(Duration::from_millis(5));
        };
        assert_eq!(found.0, ticket);
        assert!(found.1.is_ok());
    }

    #[test]
    fn wait_returns_none_when_worker_thread_died() {
        struct Exploding;
        impl crate::backend::Backend for Exploding {
            fn begin<'a>(
                &'a mut self,
                _request: &crate::backend::GenerationRequest<'_>,
            ) -> Result<Box<dyn crate::backend::Decoder + 'a>, crate::backend::BackendError>
            {
                panic!("backend exploded");
            }
        }
        let mut w = NpcWorker::spawn(|| {
            let (_, speaker) = mini();
            let mut p = parts(0, vec![speaker]);
            p.backend = Box::new(Exploding);
            Npc::new(p)
        })
        .unwrap();
        w.submit(request()).unwrap();
        assert!(w.wait().is_none());
        assert_eq!(w.submit(request()).err(), Some(WorkerGone));
    }

    #[test]
    fn drop_with_request_in_flight_joins() {
        let mut w = worker(1);
        w.submit(request()).unwrap();
        drop(w);
    }

    #[test]
    fn worker_gone_displays_a_message() {
        assert_eq!(WorkerGone.to_string(), "the NPC worker thread has stopped");
    }
}
