//! Content types, as a browser gives them and the bucket keeps them.

/// The content type of bytes nobody named.
pub const UNKNOWN: &str = "application/octet-stream";
/// The longest content type taken.
const MAX_LENGTH: usize = 255;

/// The content type a browser gave, or the unknown one when it gave none.
pub fn content_type_or_unknown(content_type: &str) -> String {
    match content_type.trim() {
        "" => UNKNOWN.to_string(),
        content_type => content_type.to_lowercase(),
    }
}

/// A `type/subtype`, with parameters or not, in printable ASCII.
pub fn is_content_type(value: &str, _context: &()) -> garde::Result {
    let essence = value.split(';').next().unwrap_or_default().trim();
    let well_formed = essence.split_once('/').is_some_and(|(kind, subtype)| {
        !kind.is_empty() && !subtype.is_empty() && !subtype.contains('/')
    });
    let printable = value.chars().all(|c| c.is_ascii_graphic() || c == ' ');
    if !well_formed || !printable || value.len() > MAX_LENGTH {
        return Err(garde::Error::new("is not a content type"));
    }
    Ok(())
}
