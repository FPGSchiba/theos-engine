//! winit → engine `KeyCode` translation table.
//!
//! Lives in `theos-window` so `theos-shared` stays free of the winit
//! dependency (see the layering note in
//! [`theos_shared::window_event`](../../theos_shared/window_event/index.html)).
//!
//! Called by [`TheosAppHandler`](../../theos_launcher/struct.TheosAppHandler.html)
//! when translating a winit `KeyboardInput` event into a
//! [`theos_shared::WindowEvent::KeyPressed`] / `KeyReleased` for publication
//! on the engine [`EventBus`](theos_shared::EventBus).

use theos_shared::window_event::{KeyCode, NativeKeyCode};
use winit::keyboard::KeyCode as WinitKeyCode;
use winit::keyboard::PhysicalKey::{Code, Unidentified};

/// Translates a winit physical-key code into the engine's
/// [`theos_shared::KeyCode`].
///
/// Any winit variant not yet mapped collapses to
/// [`KeyCode::Unidentified`] carrying [`NativeKeyCode::Unidentified`] —
/// add variants here as their consumers come online; the engine-side enum
/// already has the W3C-named slots ready.
pub fn translate_physical_key(key: winit::keyboard::PhysicalKey) -> KeyCode {
    match key {
        Code(winit_key) => match winit_key {
            WinitKeyCode::KeyA => KeyCode::KeyA,
            WinitKeyCode::KeyB => KeyCode::KeyB,
            WinitKeyCode::KeyC => KeyCode::KeyC,
            WinitKeyCode::KeyD => KeyCode::KeyD,
            WinitKeyCode::KeyE => KeyCode::KeyE,
            WinitKeyCode::KeyF => KeyCode::KeyF,
            WinitKeyCode::KeyG => KeyCode::KeyG,
            WinitKeyCode::KeyH => KeyCode::KeyH,
            WinitKeyCode::KeyI => KeyCode::KeyI,
            WinitKeyCode::KeyJ => KeyCode::KeyJ,
            WinitKeyCode::KeyK => KeyCode::KeyK,
            WinitKeyCode::KeyL => KeyCode::KeyL,
            WinitKeyCode::KeyM => KeyCode::KeyM,
            WinitKeyCode::KeyN => KeyCode::KeyN,
            WinitKeyCode::KeyO => KeyCode::KeyO,
            WinitKeyCode::KeyP => KeyCode::KeyP,
            WinitKeyCode::KeyQ => KeyCode::KeyQ,
            WinitKeyCode::KeyR => KeyCode::KeyR,
            WinitKeyCode::KeyS => KeyCode::KeyS,
            WinitKeyCode::KeyT => KeyCode::KeyT,
            WinitKeyCode::KeyU => KeyCode::KeyU,
            WinitKeyCode::KeyV => KeyCode::KeyV,
            WinitKeyCode::KeyW => KeyCode::KeyW,
            WinitKeyCode::KeyX => KeyCode::KeyX,
            WinitKeyCode::KeyY => KeyCode::KeyY,
            WinitKeyCode::KeyZ => KeyCode::KeyZ,
            WinitKeyCode::Digit0 => KeyCode::Digit0,
            WinitKeyCode::Digit1 => KeyCode::Digit1,
            WinitKeyCode::Digit2 => KeyCode::Digit2,
            WinitKeyCode::Digit3 => KeyCode::Digit3,
            WinitKeyCode::Digit4 => KeyCode::Digit4,
            WinitKeyCode::Digit5 => KeyCode::Digit5,
            WinitKeyCode::Digit6 => KeyCode::Digit6,
            WinitKeyCode::Digit7 => KeyCode::Digit7,
            WinitKeyCode::Digit8 => KeyCode::Digit8,
            WinitKeyCode::Digit9 => KeyCode::Digit9,
            WinitKeyCode::Space => KeyCode::Space,
            WinitKeyCode::Enter => KeyCode::Enter,
            WinitKeyCode::Tab => KeyCode::Tab,
            WinitKeyCode::Backspace => KeyCode::Backspace,
            WinitKeyCode::Delete => KeyCode::Delete,
            WinitKeyCode::Insert => KeyCode::Insert,
            WinitKeyCode::ArrowUp => KeyCode::ArrowUp,
            WinitKeyCode::ArrowDown => KeyCode::ArrowDown,
            WinitKeyCode::ArrowLeft => KeyCode::ArrowLeft,
            WinitKeyCode::ArrowRight => KeyCode::ArrowRight,
            WinitKeyCode::Home => KeyCode::Home,
            WinitKeyCode::End => KeyCode::End,
            WinitKeyCode::PageUp => KeyCode::PageUp,
            WinitKeyCode::PageDown => KeyCode::PageDown,
            WinitKeyCode::ShiftLeft => KeyCode::ShiftLeft,
            WinitKeyCode::ShiftRight => KeyCode::ShiftRight,
            WinitKeyCode::ControlLeft => KeyCode::ControlLeft,
            WinitKeyCode::ControlRight => KeyCode::ControlRight,
            WinitKeyCode::AltLeft => KeyCode::AltLeft,
            WinitKeyCode::AltRight => KeyCode::AltRight,
            WinitKeyCode::SuperLeft => KeyCode::SuperLeft,
            WinitKeyCode::SuperRight => KeyCode::SuperRight,
            WinitKeyCode::F1 => KeyCode::F1,
            WinitKeyCode::F2 => KeyCode::F2,
            WinitKeyCode::F3 => KeyCode::F3,
            WinitKeyCode::F4 => KeyCode::F4,
            WinitKeyCode::F5 => KeyCode::F5,
            WinitKeyCode::F6 => KeyCode::F6,
            WinitKeyCode::F7 => KeyCode::F7,
            WinitKeyCode::F8 => KeyCode::F8,
            WinitKeyCode::F9 => KeyCode::F9,
            WinitKeyCode::F10 => KeyCode::F10,
            WinitKeyCode::F11 => KeyCode::F11,
            WinitKeyCode::F12 => KeyCode::F12,
            WinitKeyCode::Escape => KeyCode::Escape,
            _ => KeyCode::Unidentified(NativeKeyCode::Unidentified),
        },
        Unidentified(nk) => KeyCode::Unidentified(translate_native(nk)),
    }
}

fn translate_native(nk: winit::keyboard::NativeKeyCode) -> NativeKeyCode {
    match nk {
        winit::keyboard::NativeKeyCode::Unidentified => NativeKeyCode::Unidentified,
        winit::keyboard::NativeKeyCode::Windows(key) => NativeKeyCode::Windows(key),
        winit::keyboard::NativeKeyCode::Android(key) => NativeKeyCode::Android(key),
        winit::keyboard::NativeKeyCode::MacOS(key) => NativeKeyCode::MacOS(key),
        winit::keyboard::NativeKeyCode::Xkb(key) => NativeKeyCode::Xkb(key),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::keyboard::{NativeKeyCode as WinitNativeKeyCode, PhysicalKey as WinitPhysicalKey};

    #[test]
    fn translate_keycode_maps_named_letters() {
        assert_eq!(
            translate_physical_key(WinitPhysicalKey::Code(WinitKeyCode::KeyA)),
            KeyCode::KeyA,
        );
        assert_eq!(
            translate_physical_key(WinitPhysicalKey::Code(WinitKeyCode::KeyZ)),
            KeyCode::KeyZ,
        );
    }

    #[test]
    fn translate_keycode_maps_escape() {
        assert_eq!(
            translate_physical_key(WinitPhysicalKey::Code(WinitKeyCode::Escape)),
            KeyCode::Escape,
        );
    }

    #[test]
    fn translate_keycode_falls_back_to_unidentified_for_unmapped_variant() {
        // `Backquote` is intentionally not in the engine-side KeyCode enum;
        // the `_` arm should collapse it to Unidentified(Unidentified).
        assert_eq!(
            translate_physical_key(WinitPhysicalKey::Code(WinitKeyCode::Backquote)),
            KeyCode::Unidentified(NativeKeyCode::Unidentified),
        );

        // A PhysicalKey::Unidentified should also pass through the
        // outer Unidentified arm cleanly.
        assert_eq!(
            translate_physical_key(WinitPhysicalKey::Unidentified(
                WinitNativeKeyCode::Unidentified
            )),
            KeyCode::Unidentified(NativeKeyCode::Unidentified),
        );
    }
}
