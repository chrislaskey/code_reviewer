//! The bar iOS shows above the keyboard in a web view (previous, next and
//! done buttons), which the pages' forms have no use for. WebKit has no
//! setting for it, so its content view's `inputAccessoryView` is replaced
//! with one that returns none, as Capacitor's `hideFormAccessoryBar` does.
//! The class is private: should a later iOS rename it, a warning is logged
//! and the bar is back; nothing breaks. Without the done button, the
//! keyboard closes by tapping outside the text box.

use objc2::ffi::class_replaceMethod;
use objc2::runtime::{AnyClass, AnyObject, Imp, Sel};
use objc2::sel;

/// Hides the bar above the keyboard in every web view of the app.
pub fn hide() {
    let Some(content_view_class) = AnyClass::get(c"WKContentView") else {
        log::warn!("WKContentView not found; the bar above the keyboard stays");
        return;
    };
    let no_accessory_view: extern "C-unwind" fn(*mut AnyObject, Sel) -> *mut AnyObject =
        no_accessory_view;
    // SAFETY: the function takes the receiver and selector and returns an
    // object, as `inputAccessoryView` does and its "@@:" encoding says;
    // `Imp` is the runtime's untyped form of such a function.
    unsafe {
        let imp: Imp = std::mem::transmute(no_accessory_view);
        class_replaceMethod(
            std::ptr::from_ref(content_view_class).cast_mut(),
            sel!(inputAccessoryView),
            imp,
            c"@@:".as_ptr(),
        );
    }
}

extern "C-unwind" fn no_accessory_view(
    _receiver: *mut AnyObject,
    _selector: Sel,
) -> *mut AnyObject {
    std::ptr::null_mut()
}
