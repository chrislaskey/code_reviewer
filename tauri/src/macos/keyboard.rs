//! Key presses sent as if from the keyboard, through Core Graphics.

use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, CGKeyCode};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

/// The virtual key code of V on an ANSI keyboard.
const KEY_V: CGKeyCode = 9;

/// Presses ⌘V as if on the keyboard, so the app in front pastes. Without
/// Accessibility access macOS drops the events silently.
pub fn press_paste_shortcut() -> Result<(), String> {
    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
        .map_err(|_| "Could not create a keyboard event source".to_string())?;
    let key_down = CGEvent::new_keyboard_event(source.clone(), KEY_V, true)
        .map_err(|_| "Could not create the key down event".to_string())?;
    let key_up = CGEvent::new_keyboard_event(source, KEY_V, false)
        .map_err(|_| "Could not create the key up event".to_string())?;
    key_down.set_flags(CGEventFlags::CGEventFlagCommand);
    key_up.set_flags(CGEventFlags::CGEventFlagCommand);
    key_down.post(CGEventTapLocation::HID);
    key_up.post(CGEventTapLocation::HID);
    Ok(())
}
