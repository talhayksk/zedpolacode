//! Method dispatch. One stdin line in, zero or one response out.
//!
//! Process I/O stays in `main.rs`; this module only maps a parsed message to a
//! response value, which is what the tests drive directly.

use serde_json::{json, Value};

use crate::card::PngRenderer;
use crate::protocol::{self, ErrorObject, Incoming, METHOD_NOT_FOUND};
use crate::tools;

pub const SERVER_NAME: &str = "zpolacode-mcp";

/// What `initialize` answers when the client does not state a version.
pub const DEFAULT_PROTOCOL_VERSION: &str = "2024-11-05";

pub struct Server<R> {
    renderer: R,
}

impl<R: PngRenderer> Server<R> {
    pub fn new(renderer: R) -> Self {
        Self { renderer }
    }

    /// Handles one line of stdin. `None` means "write nothing": notifications,
    /// including `notifications/*`, never get a response.
    pub fn handle_line(&self, line: &str) -> Option<Value> {
        match protocol::parse(line) {
            Ok(Incoming::Notification) => None,
            Ok(Incoming::Request(request)) => {
                // A client that attaches an id to `notifications/initialized`
                // or `notifications/cancelled` still gets silence.
                if request.method.starts_with("notifications/") {
                    return None;
                }
                Some(self.handle_request(&request))
            }
            Err(failure) => Some(protocol::failure(failure.id, &failure.error)),
        }
    }

    fn handle_request(&self, request: &protocol::Request) -> Value {
        let method = request.method.as_str();

        let result = match method {
            "initialize" => Ok(self.initialize(&request.params)),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(tools::list()),
            "tools/call" => tools::call(&request.params, &self.renderer),
            other => Err(ErrorObject::new(
                METHOD_NOT_FOUND,
                format!("Method not found: `{other}`."),
            )),
        };

        match result {
            Ok(result) => protocol::success(request.id.clone(), result),
            Err(error) => protocol::failure(request.id.clone(), &error),
        }
    }

    fn initialize(&self, params: &Value) -> Value {
        let version = params
            .get("protocolVersion")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .unwrap_or(DEFAULT_PROTOCOL_VERSION);

        json!({
            "protocolVersion": version,
            "capabilities": { "tools": {} },
            "serverInfo": {
                "name": SERVER_NAME,
                "version": env!("CARGO_PKG_VERSION"),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::{snapshot_line, FakeRenderer, ONE_PIXEL_PNG_BASE64};
    use crate::protocol::{INVALID_PARAMS, PARSE_ERROR};

    fn server() -> Server<FakeRenderer> {
        Server::new(FakeRenderer)
    }

    fn reply(line: &str) -> Value {
        server().handle_line(line).expect("a response")
    }

    #[test]
    fn initialize_echoes_the_client_protocol_version() {
        let response = reply(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"zed","version":"1"}}}"#,
        );

        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], json!(1));
        assert_eq!(response["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(response["result"]["capabilities"], json!({ "tools": {} }));
        assert_eq!(response["result"]["serverInfo"]["name"], "zpolacode-mcp");
        assert_eq!(
            response["result"]["serverInfo"]["version"],
            env!("CARGO_PKG_VERSION")
        );
    }

    #[test]
    fn initialize_falls_back_to_a_default_protocol_version() {
        let response = reply(r#"{"jsonrpc":"2.0","id":"a","method":"initialize"}"#);
        assert_eq!(
            response["result"]["protocolVersion"],
            DEFAULT_PROTOCOL_VERSION
        );
    }

    #[test]
    fn ping_returns_an_empty_object() {
        let response = reply(r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#);
        assert_eq!(response["result"], json!({}));
    }

    #[test]
    fn notifications_get_no_reply() {
        assert!(server()
            .handle_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
            .is_none());
        assert!(server()
            .handle_line(
                r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":1}}"#
            )
            .is_none());
        assert!(server()
            .handle_line(r#"{"jsonrpc":"2.0","id":9,"method":"notifications/initialized"}"#)
            .is_none());
    }

    #[test]
    fn an_unknown_method_names_itself() {
        let response = reply(r#"{"jsonrpc":"2.0","id":3,"method":"sampling/createMessage"}"#);
        assert_eq!(response["error"]["code"], json!(-32601));
        let message = response["error"]["message"].as_str().expect("message");
        assert!(message.contains("sampling/createMessage"), "{message}");
    }

    #[test]
    fn malformed_json_answers_with_a_null_id() {
        let response = reply("{");
        assert_eq!(response["error"]["code"], json!(PARSE_ERROR));
        assert_eq!(response["id"], Value::Null);
    }

    #[test]
    fn tools_list_exposes_one_snapshot_tool() {
        let response = reply(r#"{"jsonrpc":"2.0","id":4,"method":"tools/list"}"#);
        let tools = response["result"]["tools"].as_array().expect("tools");
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], "snapshot");
        assert_eq!(tools[0]["inputSchema"]["type"], "object");
        assert!(tools[0]["description"]
            .as_str()
            .expect("description")
            .contains("PNG"));
    }

    #[test]
    fn tools_call_returns_an_image_and_a_text_summary() {
        let response = reply(&snapshot_line(5, json!({ "code": "fn main() {}" })));

        assert!(response.get("error").is_none(), "{response}");
        let content = response["result"]["content"].as_array().expect("content");
        assert_eq!(content.len(), 2);
        assert_eq!(content[0]["type"], "image");
        assert_eq!(content[0]["mimeType"], "image/png");
        assert_eq!(content[0]["data"], ONE_PIXEL_PNG_BASE64);
        assert_eq!(content[1]["type"], "text");
        assert!(content[1]["text"]
            .as_str()
            .expect("text")
            .contains("canvas_css"));
    }

    #[test]
    fn tools_call_with_bad_arguments_is_invalid_params() {
        let response = reply(&snapshot_line(6, json!({ "code": "x", "font_size": 41 })));
        assert_eq!(response["error"]["code"], json!(INVALID_PARAMS));
        assert!(response["error"]["message"]
            .as_str()
            .expect("message")
            .contains("font_size"));
    }

    #[test]
    fn an_unknown_tool_is_invalid_params() {
        let response = reply(
            r#"{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"screenshot","arguments":{}}}"#,
        );
        assert_eq!(response["error"]["code"], json!(INVALID_PARAMS));
    }

    #[test]
    fn a_runtime_failure_is_a_tool_result_not_a_protocol_error() {
        let response = reply(&snapshot_line(8, json!({ "path": "no-such-file.rs" })));
        assert!(response.get("error").is_none(), "{response}");
        assert_eq!(response["result"]["isError"], json!(true));
    }

    /// The only test that needs a real browser. Writes the card into a temp
    /// directory so the PNG bytes can be checked without decoding base64.
    #[test]
    #[ignore = "launches a real headless browser"]
    fn real_browser_renders_a_card_png() {
        let dir = crate::browser::TempDir::create().expect("temp dir");
        let server = Server::new(crate::browser::BrowserRenderer::discover());

        let line = snapshot_line(
            11,
            json!({
                "code": "use std::collections::HashMap;\n\nfn main() {\n    let mut counts: HashMap<&str, u32> = HashMap::new();\n    *counts.entry(\"zed\").or_insert(0) += 1;\n    println!(\"{counts:?}\");\n}\n",
                "lang": "rust",
                "title": "mcp-e2e.rs",
                "write_files": true,
                "out_dir": dir.path.to_string_lossy(),
            }),
        );

        let response = server.handle_line(&line).expect("a response");
        let text = response["result"]["content"][1]["text"]
            .as_str()
            .unwrap_or("<no text>")
            .to_string();
        assert_ne!(response["result"]["isError"], json!(true), "{text}");

        let png_path = dir.path.join("mcp-e2e.png");
        let png = std::fs::read(&png_path).expect("the PNG was written");
        assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"), "not a PNG");
        assert!(
            png.len() > 1000,
            "suspiciously small PNG: {} bytes",
            png.len()
        );

        println!("png: {} bytes at {}", png.len(), png_path.display());
        println!("summary:\n{text}");
    }
}
