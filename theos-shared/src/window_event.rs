//! OS window/input events as engine-owned typed values.
//!
//! This module defines the [`WindowEvent`] enum that downstream subsystems
//! (input handling, the future `RenderModule` surface-reconfigure, gameplay
//! Escape-to-quit) subscribe to via the [`EventBus`](crate::EventBus).
//!
//! # Why a mirror enum instead of re-exporting `winit::keyboard::KeyCode`
//!
//! `theos-shared` sits below every other engine crate in the dependency DAG
//! (`theos-math`, `theos-ecs`, `theos-ai`, `theos-physics`, plugin crates, …).
//! If we re-exported `winit` types from here, every one of those crates would
//! transitively pull in winit — including headless benches, the AI worker,
//! and platform-independent math. That is a layering violation that is
//! painful to undo.
//!
//! Instead, we define an engine-owned [`KeyCode`] enum that mirrors winit's
//! variant set (which itself follows the
//! [W3C UI Events `KeyboardEvent.code`](https://www.w3.org/TR/uievents-code/)
//! naming). The translation `winit::keyboard::KeyCode → KeyCode` lives in
//! `theos-window`, where the winit dependency already exists.
//!
//! This is the same shape Bevy uses for `bevy_input::keyboard::KeyCode`,
//! and for the same reason: gameplay/AI/network code wants a stable typed
//! key vocabulary without taking on a windowing dependency.
//!
//! # Scope (THEOS-16)
//!
//! - [`WindowEvent::CloseRequested`] — user clicked the close button.
//! - [`WindowEvent::Resized`] — surface size changed; consumed by the future
//!   `RenderModule` to call `surface.configure` (out of scope here — see
//!   [THEOS-37](https://firephoenixgames.atlassian.net/browse/THEOS-37)).
//! - [`WindowEvent::KeyPressed`] / [`WindowEvent::KeyReleased`] — physical
//!   key transitions, scoped to keys that map to a known [`KeyCode`]
//!   variant. Logical (text-producing, layout-aware) keys are deferred until
//!   THEOS-47 (`InputMap` semantic actions).
//!
//! # Extending [`KeyCode`]
//!
//! The variant set covers what THEOS-16 and the first wave of consumers
//! (Escape-to-quit, WASD movement, common modifiers, function keys) need.
//! Add variants as their consumers come online — every addition must keep
//! the names in lockstep with `winit::keyboard::KeyCode` and update the
//! translation table in `theos-window`. The
//! [`KeyCode::Unidentified`](KeyCode::Unidentified) fallback exists so
//! consumers can pattern-match on it instead of dropping events on the
//! floor while the table is still growing.

use crate::event_bus::Event;

/// OS-sourced window event, published by the launcher's `TheosAppHandler`
/// onto the engine [`EventBus`](crate::EventBus).
///
/// See the [module docs](crate::window_event) for scope, layering rationale,
/// and the extension policy on [`KeyCode`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowEvent {
    /// The user requested the window be closed (close button, ⌘Q, ⌃C, etc.).
    ///
    /// The launcher's `TheosAppHandler` *also* calls `target.exit()` on this
    /// event — the bus publication is for observability, so modules that
    /// want to react during the shutdown sequence can subscribe.
    CloseRequested,

    /// The OS window's drawable surface changed dimensions.
    ///
    /// Consumed by the future `RenderModule` (THEOS-37) to call
    /// `surface.configure` with the new size. Width/height are physical
    /// pixels, not logical — DPI scaling lives in a future event.
    Resized {
        /// New surface width in physical pixels.
        width: u32,
        /// New surface height in physical pixels.
        height: u32,
    },

    /// A physical key transitioned from up to down.
    ///
    /// Only emitted for keys that map to a known [`KeyCode`] variant.
    /// Unmapped keys arrive as [`KeyCode::Unidentified`].
    KeyPressed {
        /// The physical key location (W3C UI Events `KeyboardEvent.code`).
        key: KeyCode,
    },

    /// A physical key transitioned from down to up.
    ///
    /// Mirror of [`KeyPressed`](WindowEvent::KeyPressed).
    KeyReleased {
        /// The physical key location (W3C UI Events `KeyboardEvent.code`).
        key: KeyCode,
    },
}

impl Event for WindowEvent {}

/// Engine-owned mirror of `winit::keyboard::KeyCode`.
///
/// Variant names follow the
/// [W3C UI Events `KeyboardEvent.code`](https://www.w3.org/TR/uievents-code/)
/// specification, which is what winit (and Bevy) also follow. Translation
/// from winit's enum lives in `theos-window` so this crate stays
/// windowing-free.
///
/// # Coverage
///
/// This is a curated subset of the full W3C set — sufficient for THEOS-16
/// (Escape-to-quit), WASD movement, common modifiers, function keys, and
/// arrow navigation. Keys not yet mapped arrive as
/// [`KeyCode::Unidentified`]. Add variants as consumers need them; see
/// the [module-level extension note](crate::window_event).
///
/// # Layout
///
/// `#[repr(u32)]` mirrors Bevy's choice — gives a stable discriminant for
/// debug formatting and future serialisation. Do *not* rely on the
/// numeric values matching winit's — they are independent enums.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum KeyCode {
    /// Fallback for keys not yet mapped to a named variant.
    ///
    /// Carries the platform-specific native code so a consumer can still
    /// route the key if absolutely needed (e.g., raw input recording).
    Unidentified(NativeKeyCode),

    // --- Letters (W3C: KeyA..KeyZ) ---
    /// `A` key.
    KeyA,
    /// `B` key.
    KeyB,
    /// `C` key.
    KeyC,
    /// `D` key.
    KeyD,
    /// `E` key.
    KeyE,
    /// `F` key.
    KeyF,
    /// `G` key.
    KeyG,
    /// `H` key.
    KeyH,
    /// `I` key.
    KeyI,
    /// `J` key.
    KeyJ,
    /// `K` key.
    KeyK,
    /// `L` key.
    KeyL,
    /// `M` key.
    KeyM,
    /// `N` key.
    KeyN,
    /// `O` key.
    KeyO,
    /// `P` key.
    KeyP,
    /// `Q` key.
    KeyQ,
    /// `R` key.
    KeyR,
    /// `S` key.
    KeyS,
    /// `T` key.
    KeyT,
    /// `U` key.
    KeyU,
    /// `V` key.
    KeyV,
    /// `W` key.
    KeyW,
    /// `X` key.
    KeyX,
    /// `Y` key.
    KeyY,
    /// `Z` key.
    KeyZ,

    // --- Digits (W3C: Digit0..Digit9) ---
    /// `0` key (digit row, not numpad).
    Digit0,
    /// `1` key.
    Digit1,
    /// `2` key.
    Digit2,
    /// `3` key.
    Digit3,
    /// `4` key.
    Digit4,
    /// `5` key.
    Digit5,
    /// `6` key.
    Digit6,
    /// `7` key.
    Digit7,
    /// `8` key.
    Digit8,
    /// `9` key.
    Digit9,

    // --- Whitespace & editing ---
    /// Space bar.
    Space,
    /// Enter / Return key.
    Enter,
    /// Tab key.
    Tab,
    /// Backspace key.
    Backspace,
    /// Delete (forward delete) key.
    Delete,
    /// Insert key.
    Insert,

    // --- Navigation ---
    /// Up arrow.
    ArrowUp,
    /// Down arrow.
    ArrowDown,
    /// Left arrow.
    ArrowLeft,
    /// Right arrow.
    ArrowRight,
    /// Home key.
    Home,
    /// End key.
    End,
    /// Page Up key.
    PageUp,
    /// Page Down key.
    PageDown,

    // --- Modifiers ---
    /// Left Shift.
    ShiftLeft,
    /// Right Shift.
    ShiftRight,
    /// Left Control.
    ControlLeft,
    /// Right Control.
    ControlRight,
    /// Left Alt / Option.
    AltLeft,
    /// Right Alt / Option.
    AltRight,
    /// Left Super / Windows / Command. Renamed from W3C's `MetaLeft` to
    /// match winit / Bevy.
    SuperLeft,
    /// Right Super / Windows / Command. Renamed from W3C's `MetaRight`.
    SuperRight,

    // --- Function keys ---
    /// F1.
    F1,
    /// F2.
    F2,
    /// F3.
    F3,
    /// F4.
    F4,
    /// F5.
    F5,
    /// F6.
    F6,
    /// F7.
    F7,
    /// F8.
    F8,
    /// F9.
    F9,
    /// F10.
    F10,
    /// F11.
    F11,
    /// F12.
    F12,

    // --- Special ---
    /// Escape key. Required for THEOS-16's Escape-to-quit subscriber.
    Escape,
}

/// Platform-native fallback codes carried inside
/// [`KeyCode::Unidentified`].
///
/// Mirrors the variant set of `winit::keyboard::NativeKeyCode`, so the
/// translation in `theos-window` is a 1:1 match. Consumers should treat
/// these as opaque platform-specific scancodes — they are not portable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativeKeyCode {
    /// No platform code was reported (rare; usually means the key was
    /// synthesised).
    Unidentified,
    /// Android scancode.
    Android(u32),
    /// macOS virtual keycode (`kVK_*`).
    MacOS(u16),
    /// Windows scancode.
    Windows(u16),
    /// Linux XKB keycode.
    Xkb(u32),
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn window_event_derives_compile() {
        // TODO: implement
    }

    #[test]
    fn key_code_is_copy() {
        // TODO: implement
    }

    #[test]
    fn key_code_unidentified_carries_native_code() {
        // TODO: implement
    }

    #[test]
    fn window_event_impls_event() {
        // TODO: implement
    }
}
