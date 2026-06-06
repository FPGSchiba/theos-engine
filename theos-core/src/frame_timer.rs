//! Per-frame time tracker shared between the kernel's headless driver
//! ([`crate::TheosKernel::run`]'s `variable_delta_loop`) and external
//! event-loop drivers (notably `TheosAppHandler` in `theos-launcher`, and
//! eventually the Tauri editor's in-process kernel driver).
//!
//! Each "frame" is a single call to [`FrameTimer::tick`]: it computes the
//! wall-clock delta since the previous tick, clamps it to [`MAX_DELTA_TIME`]
//! to avoid spiral-of-death on long pauses, and tracks a rolling 1-second
//! FPS average that is returned alongside the delta whenever a full second
//! has accumulated.
//!
//! # Why this lives in `theos-core`
//!
//! Duplicating the timer in every driver would mean N independent
//! implementations of the spiral-of-death clamp — a recipe for divergent
//! frame semantics. The timer is a kernel-frame concept (the kernel
//! defines what "a frame" means), so it belongs in `theos-core` rather
//! than `theos-shared`.

use std::time::Instant;

/// The clamp applied to per-frame delta times, in seconds.
///
/// If the OS pauses the process (sleep, debugger break, swapped-out window),
/// the next [`FrameTimer::tick`] could otherwise return a multi-second `dt`
/// and explode physics / animation systems that scale by `dt`. We clamp to
/// 100 ms — roughly 6 frames at 60 Hz — so the engine recovers gracefully
/// instead of attempting to "catch up" multiple seconds of simulation in a
/// single step.
pub const MAX_DELTA_TIME: f32 = 0.1;

/// The synthetic `dt` returned by the very first [`FrameTimer::tick`] call.
///
/// The first call has no previous frame to subtract from. Returning a
/// sensible 60 Hz step keeps systems that scale by `dt` (animation,
/// physics, AI command queue drain) deterministic on the first frame
/// regardless of how much wall-clock time elapsed between [`FrameTimer::new`]
/// and the first [`FrameTimer::tick`].
pub const FIRST_FRAME_DT: f32 = 1.0 / 60.0;

/// A utility for tracking frame times, calculating delta times, and measuring FPS.
pub struct FrameTimer {
    /// The timestamp when the previous frame started.
    last_frame: Instant,
    /// Whether this is the first frame being ticked.
    first_frame: bool,
    /// Time accumulated in seconds to calculate average FPS once per second.
    fps_accumulator: f32,
    /// The number of frames completed within the current second.
    fps_counter: u32,
}

impl Default for FrameTimer {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameTimer {
    /// Creates a new `FrameTimer` with `last_frame` initialized to the current time.
    pub fn new() -> Self {
        Self {
            last_frame: Instant::now(),
            first_frame: true,
            fps_accumulator: 0.0,
            fps_counter: 0,
        }
    }

    /// Ticks the timer to calculate the delta time since the last frame.
    ///
    /// The returned delta time is clamped to a maximum of `0.1` seconds to prevent
    /// the game engine from experiencing a "spiral of death" during lag spikes.
    /// On the very first frame, a default startup value of `1.0 / 60.0` is returned.
    ///
    /// # Returns
    /// A tuple of `(dt, Option<fps>)`, where `dt` is the clamped delta time in seconds,
    /// and `Some(fps)` contains the average FPS if a second has elapsed since the last FPS report.
    pub fn tick(&mut self) -> (f32, Option<f32>) {
        let now = Instant::now();
        if self.first_frame {
            self.first_frame = false;
            self.last_frame = now;
            (FIRST_FRAME_DT, None)
        } else {
            let dt = now
                .duration_since(self.last_frame)
                .as_secs_f32()
                .min(MAX_DELTA_TIME);
            self.last_frame = now;
            self.fps_accumulator += dt;
            self.fps_counter += 1;
            if self.fps_accumulator >= 1.0 {
                let fps = self.fps_counter as f32 / self.fps_accumulator;
                self.fps_accumulator = 0.0;
                self.fps_counter = 0;
                (dt, Some(fps))
            } else {
                (dt, None)
            }
        }
    }
}
