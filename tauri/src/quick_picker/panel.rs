//! The quick picker's window: a non-activating `NSPanel` (through
//! `tauri-nspanel`) that takes keystrokes while the app the person was
//! typing in stays in front and receives the paste.

use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, Size, WebviewUrl, WindowEvent, Wry};
use tauri_nspanel::{
    CollectionBehavior, ManagerExt as _, PanelBuilder, PanelHandle, PanelLevel, StyleMask,
    tauri_panel,
};

pub const LABEL: &str = "quick-picker";
const PAGE: &str = "quick-picker.html";
pub const WIDTH: f64 = 380.0;
pub const HEIGHT: f64 = 392.0;

tauri_panel! {
    panel!(QuickPickerPanel {
        config: {
            can_become_key_window: true,
            can_become_main_window: false,
            is_floating_panel: true
        }
    })
}

/// Creates the panel hidden. A panel shown in the call that creates it
/// loses key status while its page loads, so it is created at startup.
pub fn create(app: &AppHandle) -> Result<(), String> {
    PanelBuilder::<_, QuickPickerPanel>::new(app, LABEL)
        .url(WebviewUrl::App(PAGE.into()))
        .title("Quick picker")
        .size(Size::Logical(LogicalSize::new(WIDTH, HEIGHT)))
        .resizable(false)
        .level(PanelLevel::Floating)
        .floating(true)
        .has_shadow(true)
        .corner_radius(12.0)
        // A clear window behind the rounded page, else its square corners show
        .transparent(true)
        .style_mask(StyleMask::empty().borderless().nonactivating_panel())
        .collection_behavior(
            CollectionBehavior::new()
                .can_join_all_spaces()
                .full_screen_auxiliary()
                .transient(),
        )
        .no_activate(true)
        .hides_on_deactivate(false)
        .with_window(|window| {
            window
                .decorations(false)
                .skip_taskbar(true)
                .accept_first_mouse(true)
                .visible(false)
        })
        .build()
        .map_err(|error| error.to_string())?;

    // Clicking anywhere else closes the picker
    let window = app
        .get_webview_window(LABEL)
        .ok_or("the quick picker's panel has no window")?;
    let on_blur = app.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::Focused(false) = event {
            hide(&on_blur);
        }
    });
    Ok(())
}

pub fn is_open(app: &AppHandle) -> bool {
    app.get_webview_panel(LABEL)
        .is_ok_and(|panel| panel.is_visible())
}

/// Shows the panel and gives it the keyboard. Main thread only.
pub fn show(app: &AppHandle) -> Result<(), String> {
    panel(app)?.show_and_make_key();
    Ok(())
}

/// Main thread only.
pub fn hide(app: &AppHandle) {
    if let Ok(panel) = app.get_webview_panel(LABEL)
        && panel.is_visible()
    {
        panel.hide();
    }
}

/// Moves the panel's top left corner, in points from the main display's
/// top left.
pub fn set_position(app: &AppHandle, position: LogicalPosition<f64>) -> Result<(), String> {
    app.get_webview_window(LABEL)
        .ok_or("the quick picker's window is gone")?
        .set_position(position)
        .map_err(|error| error.to_string())
}

fn panel(app: &AppHandle) -> Result<PanelHandle<Wry>, String> {
    app.get_webview_panel(LABEL)
        .map_err(|error| format!("the quick picker's panel is gone: {error:?}"))
}
