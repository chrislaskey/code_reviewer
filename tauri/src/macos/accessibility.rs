//! Accessibility access, and the caret of another app through it.

use std::ffi::c_void;

use core_foundation::base::{CFType, CFTypeRef, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
use core_foundation::string::{CFString, CFStringRef};
use core_graphics::geometry::{CGPoint, CGRect, CGSize};

use super::ScreenRect;

type AXUIElementRef = CFTypeRef;
type AXValueRef = CFTypeRef;
type AXError = i32;
type Boolean = u8;

const AX_ERROR_SUCCESS: AXError = 0;
const AX_VALUE_TYPE_CGPOINT: u32 = 1;
const AX_VALUE_TYPE_CGSIZE: u32 = 2;
const AX_VALUE_TYPE_CGRECT: u32 = 3;
/// Each call is blocking IPC with the other app; the default timeout is
/// several seconds.
const CALL_TIMEOUT_SECONDS: f32 = 0.5;

// HIServices has no objc2 crate, so the functions are declared by hand.
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    static kAXTrustedCheckOptionPrompt: CFStringRef;
    fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> Boolean;
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    fn AXUIElementCopyParameterizedAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        parameter: CFTypeRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    fn AXUIElementSetAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: CFTypeRef,
    ) -> AXError;
    fn AXValueGetValue(value: AXValueRef, value_type: u32, out: *mut c_void) -> Boolean;
    fn AXUIElementSetMessagingTimeout(element: AXUIElementRef, timeout_seconds: f32) -> AXError;
}

/// Whether the user granted this app Accessibility access. With `prompt`,
/// macOS shows its dialog when access isn't granted yet. Launched from a
/// terminal, the app macOS checks is the terminal.
pub fn is_trusted(prompt: bool) -> bool {
    // SAFETY: the key is a framework constant, retained under the get rule;
    // the dictionary outlives the call.
    unsafe {
        let key = CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt);
        let options = CFDictionary::from_CFType_pairs(&[(key, CFBoolean::from(prompt))]);
        AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef()) != 0
    }
}

/// Where the app in front accepts text: the caret when the app reports it,
/// else the frame of the focused element.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TextBounds {
    Caret(ScreenRect),
    FocusedElement(ScreenRect),
}

/// The caret, else the focused element, of the app with `pid`. Native text
/// views, Safari and Firefox report the caret; Chromium and Electron apps
/// report nothing until `enable_in_app`; terminals have no caret.
pub fn focused_text_bounds(pid: i32) -> Result<TextBounds, String> {
    if !is_trusted(false) {
        return Err("no Accessibility access".into());
    }

    // SAFETY: every ref the API returns follows the create rule and is
    // wrapped in `CFType`, which releases it when dropped.
    unsafe {
        let app = application_element(pid);
        let focused = copy_attribute(&app, "AXFocusedUIElement")
            .map_err(|error| format!("no focused element: {error}"))?;

        let caret = copy_attribute(&focused, "AXSelectedTextRange")
            .and_then(|range| copy_parameterized_attribute(&focused, "AXBoundsForRange", &range))
            .and_then(|bounds| rect_value(&bounds));
        let caret_error = match caret {
            Ok(rect) if rect.x != 0.0 || rect.y != 0.0 || rect.height != 0.0 => {
                return Ok(TextBounds::Caret(rect));
            }
            Ok(_) => "empty rectangle".to_string(),
            Err(error) => error,
        };

        element_frame(&focused)
            .map(TextBounds::FocusedElement)
            .map_err(|error| format!("caret: {caret_error}; focused element: {error}"))
    }
}

/// Switches accessibility on in a Chromium or Electron app, which builds
/// its accessibility tree only once asked (`AXManualAccessibility`, from
/// Electron's accessibility guide). Other apps report it as unsupported.
pub fn enable_in_app(pid: i32) -> Result<(), String> {
    // SAFETY: as in `focused_text_bounds`; the boolean is a constant.
    unsafe {
        let app = application_element(pid);
        let attribute = CFString::new("AXManualAccessibility");
        let error = AXUIElementSetAttributeValue(
            app.as_CFTypeRef(),
            attribute.as_concrete_TypeRef(),
            CFBoolean::true_value().as_CFTypeRef(),
        );
        if error == AX_ERROR_SUCCESS {
            Ok(())
        } else {
            Err(describe_error(error))
        }
    }
}

unsafe fn application_element(pid: i32) -> CFType {
    // SAFETY: the create rule applies; the timeout call only reads the element.
    unsafe {
        let app = CFType::wrap_under_create_rule(AXUIElementCreateApplication(pid));
        AXUIElementSetMessagingTimeout(app.as_CFTypeRef(), CALL_TIMEOUT_SECONDS);
        app
    }
}

/// `AXFrame` where supported, else `AXPosition` and `AXSize`.
unsafe fn element_frame(element: &CFType) -> Result<ScreenRect, String> {
    if let Ok(frame) = unsafe { copy_attribute(element, "AXFrame") }
        && let Ok(rect) = unsafe { rect_value(&frame) }
    {
        return Ok(rect);
    }
    let position = unsafe { copy_attribute(element, "AXPosition") }?;
    let size = unsafe { copy_attribute(element, "AXSize") }?;
    let mut point = CGPoint::new(0.0, 0.0);
    let mut extent = CGSize::new(0.0, 0.0);
    // SAFETY: the out pointers match the value types asked for.
    let got_point = unsafe {
        AXValueGetValue(
            position.as_CFTypeRef(),
            AX_VALUE_TYPE_CGPOINT,
            &mut point as *mut CGPoint as *mut c_void,
        )
    };
    let got_size = unsafe {
        AXValueGetValue(
            size.as_CFTypeRef(),
            AX_VALUE_TYPE_CGSIZE,
            &mut extent as *mut CGSize as *mut c_void,
        )
    };
    if got_point == 0 || got_size == 0 {
        return Err("position or size is not a point and size".into());
    }
    Ok(ScreenRect {
        x: point.x,
        y: point.y,
        width: extent.width,
        height: extent.height,
    })
}

unsafe fn rect_value(value: &CFType) -> Result<ScreenRect, String> {
    let mut rect = CGRect::new(&CGPoint::new(0.0, 0.0), &CGSize::new(0.0, 0.0));
    // SAFETY: the out pointer is a `CGRect`, matching the type asked for.
    let got = unsafe {
        AXValueGetValue(
            value.as_CFTypeRef(),
            AX_VALUE_TYPE_CGRECT,
            &mut rect as *mut CGRect as *mut c_void,
        )
    };
    if got == 0 {
        return Err("not a rectangle".into());
    }
    Ok(ScreenRect {
        x: rect.origin.x,
        y: rect.origin.y,
        width: rect.size.width,
        height: rect.size.height,
    })
}

unsafe fn copy_attribute(element: &CFType, attribute: &str) -> Result<CFType, String> {
    let name = CFString::new(attribute);
    let mut value: CFTypeRef = std::ptr::null();
    // SAFETY: the returned ref follows the create rule.
    let error = unsafe {
        AXUIElementCopyAttributeValue(
            element.as_CFTypeRef(),
            name.as_concrete_TypeRef(),
            &mut value,
        )
    };
    if error != AX_ERROR_SUCCESS || value.is_null() {
        return Err(format!("{attribute} {}", describe_error(error)));
    }
    Ok(unsafe { CFType::wrap_under_create_rule(value) })
}

unsafe fn copy_parameterized_attribute(
    element: &CFType,
    attribute: &str,
    parameter: &CFType,
) -> Result<CFType, String> {
    let name = CFString::new(attribute);
    let mut value: CFTypeRef = std::ptr::null();
    // SAFETY: as in `copy_attribute`.
    let error = unsafe {
        AXUIElementCopyParameterizedAttributeValue(
            element.as_CFTypeRef(),
            name.as_concrete_TypeRef(),
            parameter.as_CFTypeRef(),
            &mut value,
        )
    };
    if error != AX_ERROR_SUCCESS || value.is_null() {
        return Err(format!("{attribute} {}", describe_error(error)));
    }
    Ok(unsafe { CFType::wrap_under_create_rule(value) })
}

/// The `AXError` codes in words, see `AXError.h`.
fn describe_error(error: AXError) -> String {
    match error {
        0 => "returned nothing".into(),
        -25200 => "failed".into(),
        -25201 => "illegal argument".into(),
        -25202 => "invalid element (the app or element is gone)".into(),
        -25204 => "cannot complete (the app isn't answering)".into(),
        -25205 => "unsupported by this app".into(),
        -25208 => "not implemented by this app".into(),
        -25211 => "API disabled (no Accessibility access)".into(),
        -25212 => "has no value".into(),
        -25213 => "parameterized attribute unsupported by this app".into(),
        other => format!("AXError {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accessibility_describe_error_knows_codes_and_names_the_rest() {
        assert_eq!(
            describe_error(-25211),
            "API disabled (no Accessibility access)"
        );
        assert_eq!(describe_error(-1), "AXError -1");
    }

    #[test]
    fn accessibility_focused_text_bounds_of_no_app_is_an_error() {
        if !is_trusted(false) {
            return;
        }
        let error = focused_text_bounds(i32::MAX).unwrap_err();
        assert!(error.starts_with("no focused element"), "{error}");
    }
}
