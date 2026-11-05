//! Per-key reveal decoding, with no logging or persistence.
use base64::{Engine as _, engine::general_purpose::STANDARD};

pub fn decode(encoded: &str) -> Result<String, &'static str> {
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|_| "Invalid base64 value")?;
    String::from_utf8(bytes).map_err(|_| "Binary value cannot be shown as UTF-8")
}
