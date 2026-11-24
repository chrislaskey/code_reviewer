//! iOS keeps a web view its full height when the keyboard opens, and
//! WebKit slides the visible part over the page instead. Sticky and fixed
//! elements, such as a pinned prompt, then stay where they were while the
//! page scrolls under the keyboard. So the main window's web view is made
//! shorter by the part the keyboard covers, as Capacitor's Keyboard plugin
//! does with `resize: native`, and the pages see a shorter window.

use std::ptr::NonNull;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2_core_foundation::{CGRect, CGSize};
use objc2_foundation::{NSNotification, NSNotificationCenter, NSOperationQueue, NSValue};
use objc2_ui_kit::{
    NSValueUIGeometryExtensions, UIKeyboardFrameEndUserInfoKey,
    UIKeyboardWillChangeFrameNotification, UIView,
};
use tauri::{App, Manager};

use crate::MAIN_WINDOW;

/// Resizes the main window's web view each time the keyboard opens, closes
/// or changes height, for as long as the app runs.
pub fn start(app: &App) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        log::warn!("No main window; its web view keeps its height with the keyboard open");
        return Ok(());
    };
    window.with_webview(|webview| {
        // SAFETY: on iOS `inner` is the window's WKWebView, a UIView, alive
        // while the window is; retaining it keeps it alive for the block.
        let Some(web_view) = (unsafe { Retained::retain(webview.inner().cast::<UIView>()) }) else {
            log::warn!("No web view; it keeps its height with the keyboard open");
            return;
        };
        let block = RcBlock::new(move |notification: NonNull<NSNotification>| {
            // SAFETY: the notification center passes a valid notification.
            resize(&web_view, unsafe { notification.as_ref() });
        });
        // SAFETY: the name is UIKit's constant, there is no object to
        // match, and the block runs on the main queue, where UIKit is used.
        let observer = unsafe {
            NSNotificationCenter::defaultCenter().addObserverForName_object_queue_usingBlock(
                Some(UIKeyboardWillChangeFrameNotification),
                None,
                Some(&NSOperationQueue::mainQueue()),
                &block,
            )
        };
        // The observer lasts as long as the app.
        std::mem::forget(observer);
    })
}

/// Sets the web view's height to its superview's, less the part of it the
/// keyboard will cover; a closed keyboard ends below the screen and covers none.
fn resize(web_view: &UIView, notification: &NSNotification) {
    let Some(superview) = web_view.superview() else {
        return;
    };
    let Some(user_info) = notification.userInfo() else {
        return;
    };
    // SAFETY: UIKit's constant key.
    let key = unsafe { UIKeyboardFrameEndUserInfoKey };
    let Some(keyboard_frame) = user_info
        .objectForKey(key)
        .and_then(|value| value.downcast::<NSValue>().ok())
    else {
        return;
    };
    // SAFETY: UIKit stores the keyboard's end frame as a CGRect value.
    let keyboard_frame_on_screen = unsafe { keyboard_frame.CGRectValue() };
    let keyboard_frame = superview.convertRect_fromView(keyboard_frame_on_screen, None);
    let bounds = superview.bounds();
    let bottom = bounds.origin.y + bounds.size.height;
    let covered = (bottom - keyboard_frame.origin.y).clamp(0.0, bounds.size.height);
    web_view.setFrame(CGRect {
        origin: bounds.origin,
        size: CGSize {
            width: bounds.size.width,
            height: bounds.size.height - covered,
        },
    });
}
