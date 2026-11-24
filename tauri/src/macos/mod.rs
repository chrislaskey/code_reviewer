//! macOS calls Tauri has no API for, through the `objc2-app-kit`,
//! `core-foundation` and `core-graphics` bindings: the Accessibility API,
//! running apps and keyboard events. The pasteboard is `clipboard`'s.

pub mod accessibility;
pub mod keyboard;
pub mod running_app;

/// A rectangle in screen points with the origin at the top left of the main
/// display, the same space as Tauri's logical window positions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
