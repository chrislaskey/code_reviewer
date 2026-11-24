//! Where the quick picker opens: under the text caret of the app in front, else
//! under its focused text field, else near the top of the screen the pointer
//! is on, like Spotlight.

use std::sync::mpsc;
use std::time::Duration;

use tauri::{AppHandle, LogicalPosition, Monitor};

use crate::macos::ScreenRect;
use crate::macos::accessibility::{self, TextBounds};
use crate::macos::running_app::RunningApp;

use super::panel;

/// Below the text, so the picker doesn't cover the line being typed.
const GAP_BELOW_TEXT: f64 = 6.0;
/// A focused element taller than this is an editor or a web page, not a
/// text field.
const MAX_TEXT_FIELD_HEIGHT: f64 = 80.0;
/// Where the picker's top goes on the screen, as a fraction of its height.
const SCREEN_TOP_FRACTION: f64 = 0.2;
/// How long a Chromium or Electron app gets to build its accessibility
/// tree after being asked to.
const ACCESSIBILITY_TREE_DELAY: Duration = Duration::from_millis(150);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    Caret,
    TextField,
    Screen,
}

/// The placement found and its rectangle, or why neither the caret nor a
/// text field could be used.
pub type TextPlacement = Result<(Placement, ScreenRect), String>;

/// Looks up the caret of the app in front on another thread, since the
/// Accessibility calls block until that app answers. The lookup never asks
/// this app itself: from the main thread it could only time out.
pub fn find_text(
    previous_app: Option<&RunningApp>,
    accessibility_trusted: bool,
) -> mpsc::Receiver<TextPlacement> {
    let (sender, receiver) = mpsc::channel();
    let skipped = match previous_app {
        None => Some("no app in front"),
        Some(app) if app.pid == std::process::id() as i32 => {
            Some("the picker opened over this app")
        }
        Some(_) if !accessibility_trusted => Some("no Accessibility access"),
        Some(_) => None,
    };
    match (skipped, previous_app) {
        (None, Some(app)) => {
            let app = app.clone();
            std::thread::spawn(move || {
                let _ = sender.send(text_placement(&app));
            });
        }
        (reason, _) => {
            let _ = sender.send(Err(reason.unwrap_or_default().to_string()));
        }
    }
    receiver
}

/// Chromium and Electron apps answer only after accessibility is switched
/// on in them, so a failed first look asks for that and looks again.
fn text_placement(app: &RunningApp) -> TextPlacement {
    let bounds = match accessibility::focused_text_bounds(app.pid) {
        Ok(bounds) => bounds,
        Err(first) => {
            accessibility::enable_in_app(app.pid)
                .map_err(|error| format!("{first}; enabling accessibility: {error}"))?;
            std::thread::sleep(ACCESSIBILITY_TREE_DELAY);
            accessibility::focused_text_bounds(app.pid)
                .map_err(|retry| format!("{first}; after enabling accessibility: {retry}"))?
        }
    };
    placement_for(bounds)
}

fn placement_for(bounds: TextBounds) -> TextPlacement {
    match bounds {
        TextBounds::Caret(rect) => Ok((Placement::Caret, rect)),
        TextBounds::FocusedElement(rect) if rect.height <= MAX_TEXT_FIELD_HEIGHT => {
            Ok((Placement::TextField, rect))
        }
        TextBounds::FocusedElement(rect) => Err(format!(
            "no caret, and the focused element is {}×{} points, too big for a text field",
            rect.width.round(),
            rect.height.round()
        )),
    }
}

pub fn position_below(text: &ScreenRect) -> LogicalPosition<f64> {
    LogicalPosition::new(text.x, text.y + text.height + GAP_BELOW_TEXT)
}

/// Centered in the upper part of the screen.
pub fn position_on_screen(screen: &ScreenRect) -> LogicalPosition<f64> {
    LogicalPosition::new(
        screen.x + (screen.width - panel::WIDTH) / 2.0,
        screen.y + screen.height * SCREEN_TOP_FRACTION,
    )
}

/// The nearest position at which the whole picker is on the screen.
pub fn keep_on_screen(position: LogicalPosition<f64>, screen: &ScreenRect) -> LogicalPosition<f64> {
    let x = position
        .x
        .min(screen.x + screen.width - panel::WIDTH)
        .max(screen.x);
    let y = position
        .y
        .min(screen.y + screen.height - panel::HEIGHT)
        .max(screen.y);
    LogicalPosition::new(x, y)
}

/// The usable area of the screen the pointer is on, else of the main one.
pub fn pointer_screen(app: &AppHandle) -> Result<ScreenRect, String> {
    let primary = app
        .primary_monitor()
        .map_err(|error| error.to_string())?
        .ok_or("no main display")?;
    // Tauri reports the pointer in the main display's physical pixels.
    let pointer = app
        .cursor_position()
        .map_err(|error| error.to_string())?
        .to_logical(primary.scale_factor());
    Ok(screen_at(app, pointer).unwrap_or_else(|| screen_rect(&primary)))
}

/// The usable area of the screen containing `position`, in points.
pub fn screen_at(app: &AppHandle, position: LogicalPosition<f64>) -> Option<ScreenRect> {
    // On macOS Tauri's `monitor_from_point` takes points, unlike `Monitor`'s
    // position, size and work area, which are physical pixels.
    app.monitor_from_point(position.x, position.y)
        .ok()
        .flatten()
        .map(|monitor| screen_rect(&monitor))
}

fn screen_rect(monitor: &Monitor) -> ScreenRect {
    let area = monitor.work_area();
    let scale = monitor.scale_factor();
    ScreenRect {
        x: f64::from(area.position.x) / scale,
        y: f64::from(area.position.y) / scale,
        width: f64::from(area.size.width) / scale,
        height: f64::from(area.size.height) / scale,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: ScreenRect = ScreenRect {
        x: 0.0,
        y: 25.0,
        width: 1440.0,
        height: 875.0,
    };

    fn rect(x: f64, y: f64, width: f64, height: f64) -> ScreenRect {
        ScreenRect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn quick_picker_placement_for_a_caret_is_the_caret() {
        let caret = rect(100.0, 200.0, 1.0, 16.0);
        assert_eq!(
            placement_for(TextBounds::Caret(caret)),
            Ok((Placement::Caret, caret))
        );
    }

    #[test]
    fn quick_picker_placement_for_a_short_focused_element_is_a_text_field() {
        let field = rect(100.0, 200.0, 300.0, 80.0);
        assert_eq!(
            placement_for(TextBounds::FocusedElement(field)),
            Ok((Placement::TextField, field))
        );
    }

    #[test]
    fn quick_picker_placement_for_a_tall_focused_element_is_refused_with_its_size() {
        let page = rect(0.0, 0.0, 1728.0, 1084.4);
        let error = placement_for(TextBounds::FocusedElement(page)).unwrap_err();
        assert_eq!(
            error,
            "no caret, and the focused element is 1728×1084 points, too big for a text field"
        );
    }

    #[test]
    fn quick_picker_position_below_leaves_a_gap_under_the_text() {
        let position = position_below(&rect(100.0, 200.0, 1.0, 16.0));
        assert_eq!((position.x, position.y), (100.0, 222.0));
    }

    #[test]
    fn quick_picker_position_on_screen_is_centered_in_the_upper_part() {
        let position = position_on_screen(&SCREEN);
        assert_eq!((position.x, position.y), (530.0, 200.0));
    }

    #[test]
    fn quick_picker_keep_on_screen_moves_a_position_inside() {
        let inside = keep_on_screen(LogicalPosition::new(100.0, 100.0), &SCREEN);
        assert_eq!((inside.x, inside.y), (100.0, 100.0));

        let past_the_bottom_right = keep_on_screen(LogicalPosition::new(1400.0, 1500.0), &SCREEN);
        assert_eq!(
            (past_the_bottom_right.x, past_the_bottom_right.y),
            (1060.0, 508.0)
        );

        let past_the_top_left = keep_on_screen(LogicalPosition::new(-50.0, 0.0), &SCREEN);
        assert_eq!((past_the_top_left.x, past_the_top_left.y), (0.0, 25.0));
    }

    #[test]
    fn quick_picker_find_text_skips_the_lookup_without_an_app_or_access() {
        let this_app = RunningApp {
            pid: std::process::id() as i32,
            name: "desktop".into(),
        };
        let other_app = RunningApp {
            pid: 1,
            name: "launchd".into(),
        };
        assert_eq!(
            find_text(None, true).recv(),
            Ok(Err("no app in front".into()))
        );
        assert_eq!(
            find_text(Some(&this_app), true).recv(),
            Ok(Err("the picker opened over this app".into()))
        );
        assert_eq!(
            find_text(Some(&other_app), false).recv(),
            Ok(Err("no Accessibility access".into()))
        );
    }
}
