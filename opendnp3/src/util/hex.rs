//! Hex string helpers used in tests and logging.

use crate::error::ParseError;

/// Decode a hex string like `"C0 DE 12"` or `"C0DE12"` into bytes.
pub fn from_hex(s: &str) -> Result<Vec<u8>, ParseError> {
    let filtered: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if !filtered.len().is_multiple_of(2) {
        return Err(ParseError::BadLength);
    }
    (0..filtered.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&filtered[i..i + 2], 16).map_err(|_| ParseError::BadHeader))
        .collect()
}

/// Encode bytes as an uppercase space-separated hex string, e.g. `C0 DE`.
pub fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(bytes.len() * 3);
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0F) as usize] as char);
    }
    out
}
