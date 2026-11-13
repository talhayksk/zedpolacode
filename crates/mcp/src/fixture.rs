//! Test fixtures shared by the unit tests.
//!
//! `ONE_PIXEL_PNG` is a 1×1 RGBA PNG and `ONE_PIXEL_PNG_BASE64` is its base64
//! encoding. Both were produced and verified outside this crate (`node` with
//! `zlib.crc32` checked the chunk CRCs), so the encoder is never tested with its
//! own output.

use serde_json::{json, Value};

use crate::card::{PngRenderer, RenderError};

pub const ONE_PIXEL_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae,
    0x42, 0x60, 0x82,
];

pub const ONE_PIXEL_PNG_BASE64: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAACklEQVR4nGMAAQAABQABDQottAAAAABJRU5ErkJggg==";

/// Returns a fixed PNG, so protocol tests never launch a browser.
pub struct FakeRenderer;

impl PngRenderer for FakeRenderer {
    fn render_png(
        &self,
        _html: &str,
        _width: u32,
        _height: u32,
        _scale: u32,
    ) -> Result<Vec<u8>, RenderError> {
        Ok(ONE_PIXEL_PNG.to_vec())
    }
}

/// A `tools/call` line that asks for a `snapshot`.
pub fn snapshot_line(id: u64, arguments: Value) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": { "name": "snapshot", "arguments": arguments },
    })
    .to_string()
}
