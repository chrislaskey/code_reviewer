//! The iOS clipboard: one pasteboard item with the image's own bytes and
//! a PNG rendering. No file URL: the app's files are its own, which no
//! other app reads.

use objc2::AnyThread as _;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_foundation::{NSArray, NSData, NSDictionary, NSString};
use objc2_ui_kit::{UIImage, UIPasteboard};

use super::{ClipboardImage, PNG_TYPE};

pub(super) fn copy_image(image: ClipboardImage<'_>) -> Result<Vec<String>, String> {
    write_image(image, &UIPasteboard::generalPasteboard())
}

fn write_image(
    image: ClipboardImage<'_>,
    pasteboard: &UIPasteboard,
) -> Result<Vec<String>, String> {
    let bytes = NSData::with_bytes(image.bytes);
    let rendered = UIImage::initWithData(UIImage::alloc(), &bytes)
        .ok_or("this image cannot be read to copy it")?;

    let mut types: Vec<Retained<NSString>> = vec![NSString::from_str(image.uniform_type)];
    let mut representations: Vec<Retained<AnyObject>> = vec![bytes.into()];
    if image.uniform_type != PNG_TYPE
        && let Some(png) = rendered.png_representation()
    {
        types.push(NSString::from_str(PNG_TYPE));
        representations.push(png.into());
    }

    let keys: Vec<&NSString> = types.iter().map(|key| &**key).collect();
    let objects: Vec<&AnyObject> = representations.iter().map(|object| &**object).collect();
    let item = NSDictionary::from_slices(&keys, &objects);
    // SAFETY: each value is NSData, the type a pasteboard item holds
    // for these types.
    unsafe { pasteboard.setItems(&NSArray::from_retained_slice(&[item])) };
    Ok(types.iter().map(|key| key.to_string()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clipboard::tests::GIF;

    /// On a pasteboard of the test's own, so the person's clipboard is
    /// left alone.
    fn written(
        uniform_type: &str,
        bytes: &[u8],
    ) -> Result<(Retained<UIPasteboard>, Vec<String>), String> {
        let folder = tempfile::tempdir().expect("a folder");
        let file = folder.path().join("image");
        std::fs::write(&file, bytes).expect("a file");
        let pasteboard = UIPasteboard::pasteboardWithUniqueName();
        let image = ClipboardImage {
            bytes,
            uniform_type,
            file: &file,
        };
        let written = write_image(image, &pasteboard)?;
        Ok((pasteboard, written))
    }

    #[test]
    fn clipboard_ios_copy_image_writes_a_gif_as_its_bytes_and_png() {
        let (pasteboard, written) = written("com.compuserve.gif", GIF).expect("copied");
        assert_eq!(written, ["com.compuserve.gif", "public.png"]);
        let gif = pasteboard
            .dataForPasteboardType(&NSString::from_str("com.compuserve.gif"))
            .expect("the GIF")
            .to_vec();
        assert_eq!(gif, GIF, "the animation is kept");
        let png = pasteboard
            .dataForPasteboardType(&NSString::from_str(PNG_TYPE))
            .expect("a PNG")
            .to_vec();
        assert_eq!(&png[..4], b"\x89PNG");
    }

    #[test]
    fn clipboard_ios_copy_image_refuses_bytes_that_are_no_image() {
        let error = written("public.png", b"hello").expect_err("not an image");
        assert!(error.contains("cannot be read"), "{error}");
    }
}
