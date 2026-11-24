//! Copying an image to the system clipboard on Apple's platforms, where
//! Tauri has nothing that keeps an animated GIF: each app pastes the form
//! it understands, the GIF's own bytes where GIFs are taken and a PNG
//! elsewhere. AppKit on macOS (`macos`), UIKit on iOS (`ios`).

#[cfg(target_os = "ios")]
mod ios;
#[cfg(target_os = "macos")]
mod macos;

use std::path::Path;

const PNG_TYPE: &str = "public.png";

/// An image to copy: its encoded bytes, their uniform type identifier,
/// see `uniform_type_of`, and the file on this device with those bytes.
pub struct ClipboardImage<'a> {
    pub bytes: &'a [u8],
    pub uniform_type: &'a str,
    #[cfg_attr(
        target_os = "ios",
        expect(dead_code, reason = "no other iOS app reads the app's files")
    )]
    pub file: &'a Path,
}

/// The image content types the clipboard takes, each with the uniform
/// type identifier Apple's platforms know it by.
const IMAGE_TYPES: &[(&str, &str)] = &[
    ("image/png", PNG_TYPE),
    ("image/jpeg", "public.jpeg"),
    ("image/gif", "com.compuserve.gif"),
    ("image/webp", "org.webmproject.webp"),
    ("image/tiff", "public.tiff"),
    ("image/bmp", "com.microsoft.bmp"),
    ("image/heic", "public.heic"),
];

/// The uniform type identifier Apple's platforms know an image content
/// type by, for those the clipboard takes.
pub fn uniform_type_of(content_type: &str) -> Option<&'static str> {
    let essence = content_type.split(';').next().unwrap_or_default().trim();
    IMAGE_TYPES
        .iter()
        .find(|(image_type, _uniform_type)| *image_type == essence)
        .map(|(_image_type, uniform_type)| *uniform_type)
}

/// The image content types the clipboard takes, as a search asks for them.
#[cfg(target_os = "macos")]
pub fn image_content_types() -> Vec<String> {
    IMAGE_TYPES
        .iter()
        .map(|(image_type, _uniform_type)| image_type.to_string())
        .collect()
}

/// Copies the image to the clipboard; call on the main thread, where
/// the platform's pasteboard and images live. Returns the types written.
pub fn copy_image(image: ClipboardImage<'_>) -> Result<Vec<String>, String> {
    #[cfg(target_os = "macos")]
    return macos::copy_image(image);
    #[cfg(target_os = "ios")]
    return ios::copy_image(image);
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A GIF of one white pixel.
    pub(crate) const GIF: &[u8] = &[
        0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 0x01, 0x00, 0x01, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00,
        0x00, 0xff, 0xff, 0xff, 0x21, 0xf9, 0x04, 0x01, 0x00, 0x00, 0x00, 0x00, 0x2c, 0x00, 0x00,
        0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x02, 0x02, 0x44, 0x01, 0x00, 0x3b,
    ];

    #[test]
    fn clipboard_uniform_type_of_knows_the_images_it_copies() {
        assert_eq!(uniform_type_of("image/gif"), Some("com.compuserve.gif"));
        assert_eq!(uniform_type_of("image/png; charset=binary"), Some(PNG_TYPE));
        assert_eq!(uniform_type_of("image/svg+xml"), None);
    }
}
