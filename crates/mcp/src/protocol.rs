//! JSON-RPC 2.0 envelopes for the MCP stdio transport.
//!
//! One JSON object per line on stdin, at most one JSON object per line on
//! stdout. Nothing here touches stdin or stdout: this module only turns a line
//! into a request or notification and wraps results into responses, so the
//! protocol can be exercised without a process.

use serde_json::{json, Value};

pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;

#[derive(Debug)]
pub struct ErrorObject {
    pub code: i64,
    pub message: String,
}

impl ErrorObject {
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

#[derive(Debug)]
pub struct Request {
    /// As sent: a string or a number. `null` is treated as "no id", which makes
    /// the message a notification.
    pub id: Value,
    pub method: String,
    pub params: Value,
}

#[derive(Debug)]
pub enum Incoming {
    Request(Request),
    /// An id-less request. Which notification it is does not matter: nothing
    /// answers any of them.
    Notification,
}

/// A line that could not be turned into a request. `id` is `null` when the
/// envelope was too broken to read one.
#[derive(Debug)]
pub struct ParseError {
    pub id: Value,
    pub error: ErrorObject,
}

pub fn parse(line: &str) -> Result<Incoming, ParseError> {
    let value: Value = serde_json::from_str(line).map_err(|error| ParseError {
        id: Value::Null,
        error: ErrorObject::new(PARSE_ERROR, format!("Parse error: {error}.")),
    })?;

    let object = value.as_object().ok_or_else(|| ParseError {
        id: Value::Null,
        error: ErrorObject::new(INVALID_REQUEST, "Invalid request: expected a JSON object."),
    })?;

    let id = object
        .get("id")
        .filter(|id| !id.is_null())
        .cloned()
        .unwrap_or(Value::Null);

    let method = match object.get("method").and_then(Value::as_str) {
        Some(method) => method.to_string(),
        None => {
            return Err(ParseError {
                id,
                error: ErrorObject::new(
                    INVALID_REQUEST,
                    "Invalid request: `method` must be a string.",
                ),
            })
        }
    };

    let params = object.get("params").cloned().unwrap_or(Value::Null);

    if id.is_null() {
        return Ok(Incoming::Notification);
    }
    Ok(Incoming::Request(Request { id, method, params }))
}

pub fn success(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

pub fn failure(id: Value, error: &ErrorObject) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": error.code, "message": error.message },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_request() {
        let incoming = parse(r#"{"jsonrpc":"2.0","id":7,"method":"ping","params":{}}"#).unwrap();
        match incoming {
            Incoming::Request(request) => {
                assert_eq!(request.id, json!(7));
                assert_eq!(request.method, "ping");
                assert_eq!(request.params, json!({}));
            }
            other => panic!("expected a request, got {other:?}"),
        }
    }

    #[test]
    fn parses_a_notification() {
        let incoming = parse(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).unwrap();
        assert!(matches!(incoming, Incoming::Notification));
    }

    #[test]
    fn treats_a_null_id_as_a_notification() {
        assert!(matches!(
            parse(r#"{"jsonrpc":"2.0","id":null,"method":"ping"}"#),
            Ok(Incoming::Notification)
        ));
    }

    #[test]
    fn reports_malformed_json_with_a_null_id() {
        let error = parse("{ not json").unwrap_err();
        assert_eq!(error.error.code, PARSE_ERROR);
        assert_eq!(error.id, Value::Null);
    }

    #[test]
    fn rejects_a_non_object_envelope() {
        let error = parse("[]").unwrap_err();
        assert_eq!(error.error.code, INVALID_REQUEST);
    }

    #[test]
    fn keeps_the_id_when_the_method_is_missing() {
        let error = parse(r#"{"jsonrpc":"2.0","id":"a"}"#).unwrap_err();
        assert_eq!(error.error.code, INVALID_REQUEST);
        assert_eq!(error.id, json!("a"));
    }

    #[test]
    fn wraps_results_and_errors() {
        assert_eq!(
            success(json!(1), json!({})),
            json!({"jsonrpc":"2.0","id":1,"result":{}})
        );
        assert_eq!(
            failure(
                Value::Null,
                &ErrorObject::new(METHOD_NOT_FOUND, "Method not found: `x`.")
            ),
            json!({"jsonrpc":"2.0","id":null,"error":{"code":-32601,"message":"Method not found: `x`."}})
        );
    }
}
