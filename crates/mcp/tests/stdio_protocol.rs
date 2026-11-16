//! Integration tests for the `zpolacode-mcp` stdio transport.
//!
//! Every test here spawns the real binary and talks to it over pipes, so the
//! assertions describe exactly what a client sees: one JSON object per line on
//! stdout, diagnostics on stderr, exit code, and the JSON-RPC envelopes.
//!
//! None of these tests needs a browser. Argument and runtime failures are
//! reached before the browser is spawned, and the only test that sets
//! `POLACODE_BROWSER` points it at a file that cannot run.

use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

/// Cargo builds the binary before it runs integration tests.
const BIN: &str = env!("CARGO_BIN_EXE_zpolacode-mcp");

/// Generous per-session budget. Nothing here should take a second; a value this
/// large only exists so a regression fails instead of hanging the suite.
const BUDGET: Duration = Duration::from_secs(30);

struct Session {
    code: Option<i32>,
    stdout: String,
    stderr: String,
    elapsed: Duration,
}

fn run_with_env(input: &[u8], envs: &[(&str, &str)]) -> Session {
    let mut command = Command::new(BIN);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in envs {
        command.env(key, value);
    }

    let mut child = command.spawn().expect("the server binary starts");
    let started = Instant::now();

    let mut stdin = child.stdin.take().expect("stdin");
    let payload = input.to_vec();
    // The writer runs on its own thread: a multi-megabyte payload cannot be
    // handed to a blocked pipe while the parent still needs to drain stdout.
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(&payload);
        // Dropping stdin closes the pipe, which is what EOF means to the server.
    });

    let reader = spawn_reader(child.stdout.take().expect("stdout"));
    let err_reader = spawn_reader(child.stderr.take().expect("stderr"));

    let code = wait_with_budget(&mut child);
    let elapsed = started.elapsed();

    writer.join().expect("the writer thread finishes");
    let out = reader.join().expect("the stdout thread finishes");
    let err = err_reader.join().expect("the stderr thread finishes");

    Session {
        code,
        stdout: String::from_utf8(out).expect("stdout is valid UTF-8"),
        stderr: String::from_utf8(err).expect("stderr is valid UTF-8"),
        elapsed,
    }
}

fn run(input: &[u8]) -> Session {
    run_with_env(input, &[])
}

fn spawn_reader(source: impl Read + Send + 'static) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut buffer = Vec::new();
        let mut source = source;
        let _ = source.read_to_end(&mut buffer);
        buffer
    })
}

fn wait_with_budget(child: &mut Child) -> Option<i32> {
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().expect("try_wait") {
            return status.code();
        }
        if started.elapsed() > BUDGET {
            let _ = child.kill();
            panic!("the server did not exit within {BUDGET:?}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The stdout contract: LF-separated JSON objects, nothing else.
fn responses(session: &Session) -> Vec<Value> {
    assert_eq!(
        session.code,
        Some(0),
        "exit code; stderr was: {}",
        session.stderr
    );
    assert!(
        session.stdout.is_empty() || session.stdout.ends_with('\n'),
        "every message is newline-terminated: {:?}",
        session.stdout
    );
    assert!(
        !session.stdout.contains('\r'),
        "stdout must use bare LF, not CRLF"
    );

    let mut parsed = Vec::new();
    for line in session.stdout.lines() {
        assert!(
            !line.trim().is_empty(),
            "stdout must not contain blank lines: {:?}",
            session.stdout
        );
        let value: Value = serde_json::from_str(line)
            .unwrap_or_else(|error| panic!("stdout line is not JSON: {line} ({error})"));
        assert!(value.is_object(), "stdout line is not an object: {line}");
        assert_eq!(value["jsonrpc"], "2.0", "missing jsonrpc 2.0: {line}");
        assert!(
            value.get("result").is_some() != value.get("error").is_some(),
            "a response carries exactly one of result and error: {line}"
        );
        parsed.push(value);
    }
    parsed
}

/// Feeds one JSON object per line to a fresh server process.
fn pipes(lines: &[Value]) -> Session {
    let mut input = String::new();
    for line in lines {
        input.push_str(&serde_json::to_string(line).expect("encode"));
        input.push('\n');
    }
    run(input.as_bytes())
}

fn request(id: Value, method: &str, params: Option<Value>) -> Value {
    let mut message = json!({ "jsonrpc": "2.0", "id": id, "method": method });
    if let Some(params) = params {
        message["params"] = params;
    }
    message
}

fn snapshot_call(id: Value, arguments: Value) -> Value {
    request(
        id,
        "tools/call",
        Some(json!({ "name": "snapshot", "arguments": arguments })),
    )
}

#[test]
fn initialize_echoes_the_client_protocol_version() {
    let response = responses(&pipes(&[request(
        json!(1),
        "initialize",
        Some(json!({
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": { "name": "qa", "version": "1" }
        })),
    )]));

    assert_eq!(response.len(), 1);
    assert_eq!(response[0]["id"], json!(1));
    assert_eq!(response[0]["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(
        response[0]["result"]["capabilities"],
        json!({ "tools": {} })
    );
    assert_eq!(response[0]["result"]["serverInfo"]["name"], "zpolacode-mcp");
    assert_eq!(
        response[0]["result"]["serverInfo"]["version"],
        env!("CARGO_PKG_VERSION")
    );
}

#[test]
fn initialize_falls_back_when_the_version_is_missing_or_empty() {
    let response = responses(&pipes(&[
        request(json!("a"), "initialize", None),
        request(
            json!("b"),
            "initialize",
            Some(json!({ "protocolVersion": "" })),
        ),
        // A non-string version cannot be echoed; the default is used instead.
        request(
            json!("c"),
            "initialize",
            Some(json!({ "protocolVersion": 123 })),
        ),
        // A non-object `params` is tolerated here; only `tools/call` rejects it.
        request(json!("d"), "initialize", Some(json!("not-an-object"))),
    ]));

    assert_eq!(response.len(), 4);
    for reply in &response {
        assert_eq!(reply["result"]["protocolVersion"], "2024-11-05");
    }
}

#[test]
fn one_json_object_per_line_and_nothing_else() {
    let session = pipes(&[
        request(json!(1), "initialize", Some(json!({}))),
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
        request(json!(2), "ping", None),
        request(json!(3), "tools/list", None),
        snapshot_call(json!(4), json!({ "path": "definitely-not-here.rs" })),
        request(json!(5), "sampling/createMessage", None),
    ]);

    let response = responses(&session);
    assert_eq!(
        response.len(),
        5,
        "one response per request, none per notification"
    );
    assert!(
        session.stderr.is_empty(),
        "a clean session writes nothing to stderr: {}",
        session.stderr
    );
    // The image-free case still has to be a single line even with a text block.
    assert_eq!(response[4]["error"]["code"], json!(-32601));
}

#[test]
fn ids_are_echoed_exactly() {
    let response = responses(&pipes(&[
        request(json!("ses-abc"), "ping", None),
        request(json!(0), "ping", None),
        request(json!(9007199254740991i64), "ping", None),
        request(json!(-7), "ping", None),
    ]));

    assert_eq!(response.len(), 4);
    assert_eq!(response[0]["id"], json!("ses-abc"));
    assert_eq!(response[1]["id"], json!(0));
    assert_eq!(response[2]["id"], json!(9007199254740991i64));
    assert_eq!(response[3]["id"], json!(-7));
    for reply in &response {
        assert_eq!(reply["result"], json!({}));
    }
}

#[test]
fn notifications_and_a_null_id_get_no_reply() {
    let session = pipes(&[
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
        json!({
            "jsonrpc": "2.0",
            "method": "notifications/cancelled",
            "params": { "requestId": 1 }
        }),
        json!({ "jsonrpc": "2.0", "id": null, "method": "ping" }),
        json!({ "jsonrpc": "2.0", "method": "tools/call", "params": null }),
    ]);

    let response = responses(&session);
    assert!(
        response.is_empty(),
        "notifications get no reply: {response:?}"
    );
    assert!(session.stdout.is_empty(), "nothing at all on stdout");
}

#[test]
fn an_unknown_method_is_method_not_found() {
    let response = responses(&pipes(&[request(
        json!("m-1"),
        "sampling/createMessage",
        None,
    )]));

    assert_eq!(response[0]["error"]["code"], json!(-32601));
    assert_eq!(response[0]["id"], json!("m-1"));
    let message = response[0]["error"]["message"].as_str().expect("message");
    assert!(message.contains("sampling/createMessage"), "{message}");
}

#[test]
fn malformed_json_is_a_parse_error_with_a_null_id() {
    let session = run(b"{ not json\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"ping\"}\n");
    let response = responses(&session);

    assert_eq!(response.len(), 2);
    assert_eq!(response[0]["error"]["code"], json!(-32700));
    assert_eq!(response[0]["id"], Value::Null);
    // A parse error does not stop the loop.
    assert_eq!(response[1]["id"], json!(2));
}

#[test]
fn a_non_object_envelope_is_invalid_request() {
    let session = run(b"[]\n\"ping\"\n123\n");
    let response = responses(&session);

    assert_eq!(response.len(), 3);
    for reply in &response {
        assert_eq!(reply["error"]["code"], json!(-32600), "{reply}");
        assert_eq!(reply["id"], Value::Null);
    }
}

#[test]
fn bad_arguments_are_invalid_params() {
    let cases = vec![
        snapshot_call(json!(1), json!({ "code": "x", "font_size": 41 })),
        snapshot_call(json!(2), json!({ "code": "x", "font_size": "big" })),
        snapshot_call(json!(3), json!({ "code": "x", "colour": "red" })),
        snapshot_call(json!(4), json!({ "code": "x", "bg": "chartreuse" })),
        snapshot_call(json!(5), json!({ "code": "x", "theme": "nope" })),
        snapshot_call(json!(6), json!({ "path": "a.rs", "code": "x" })),
        snapshot_call(json!(7), json!({})),
        snapshot_call(json!(8), json!({ "code": "x", "lines": "9-4" })),
        snapshot_call(json!(9), json!({ "code": "x", "write_files": true })),
        snapshot_call(json!(10), json!({ "code": "x", "padding": 121 })),
        snapshot_call(json!(11), json!({ "code": "x", "radius": 49 })),
        snapshot_call(json!(12), json!({ "code": "x", "scale": 0 })),
        // `arguments` not an object.
        snapshot_call(json!(13), json!("not-an-object")),
        // No `arguments` at all is the same as `{}`.
        request(json!(14), "tools/call", Some(json!({ "name": "snapshot" }))),
        // `params` not an object.
        request(json!(15), "tools/call", Some(json!("nope"))),
        // Unknown tool.
        request(
            json!(16),
            "tools/call",
            Some(json!({ "name": "screenshot", "arguments": {} })),
        ),
    ];

    let response = responses(&pipes(&cases));
    assert_eq!(response.len(), cases.len());
    for (index, reply) in response.iter().enumerate() {
        assert_eq!(
            reply["error"]["code"],
            json!(-32602),
            "case {index} is invalid params, not a tool result: {reply}"
        );
        assert!(
            reply.get("result").is_none(),
            "case {index} must not carry a result: {reply}"
        );
    }
}

#[test]
fn runtime_failures_are_tool_results_with_is_error() {
    let dir = std::env::temp_dir().join(format!("polacode-qa-fixtures-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("fixture dir");
    let binary = dir.join("binary.bin");
    std::fs::write(&binary, [0x00u8, 0x80, 0xff, 0xfe, 0x01]).expect("write");
    let empty = dir.join("empty.txt");
    std::fs::write(&empty, b"").expect("write");

    let response = responses(&pipes(&[
        snapshot_call(json!(1), json!({ "path": "definitely-not-here.rs" })),
        snapshot_call(json!(2), json!({ "path": dir.to_string_lossy() })),
        snapshot_call(json!(3), json!({ "path": binary.to_string_lossy() })),
        snapshot_call(json!(4), json!({ "path": empty.to_string_lossy() })),
        // A start line past EOF is a runtime failure, not bad arguments.
        snapshot_call(
            json!(5),
            json!({ "path": empty.to_string_lossy(), "lines": "9-12" }),
        ),
    ]));

    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(response.len(), 5);
    for (index, reply) in response.iter().enumerate() {
        assert!(
            reply.get("error").is_none(),
            "case {index} is a tool result, not a protocol error: {reply}"
        );
        assert_eq!(reply["result"]["isError"], json!(true), "case {index}");
        let content = reply["result"]["content"].as_array().expect("content");
        assert_eq!(content.len(), 1, "case {index} has one text block");
        assert_eq!(content[0]["type"], "text", "case {index}");
        assert!(
            !content[0]["text"].as_str().unwrap_or("").is_empty(),
            "case {index} explains itself"
        );
        assert!(
            content[0].get("data").is_none(),
            "case {index} must not carry an image"
        );
    }
}

#[test]
fn blank_and_whitespace_lines_are_skipped() {
    let session = run(b"\n   \n\t\n\r\n{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n\n");
    let response = responses(&session);

    assert_eq!(response.len(), 1);
    assert_eq!(response[0]["id"], json!(1));
}

#[test]
fn crlf_line_endings_are_accepted() {
    let session = run(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\r\n\
          {\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"ping\"}\r\n");
    let response = responses(&session);

    assert_eq!(response.len(), 2);
    assert_eq!(response[0]["id"], json!(1));
    assert_eq!(response[1]["id"], json!(2));
}

#[test]
fn a_utf8_bom_is_rejected_with_a_parse_error() {
    // JSON-RPC 2.0 and the MCP stdio transport both forbid a BOM, so rejecting
    // it is correct; this test documents the observable result for clients.
    let mut input = vec![0xEF, 0xBB, 0xBF];
    input.extend_from_slice(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n");

    let response = responses(&run(&input));
    assert_eq!(response.len(), 1);
    assert_eq!(response[0]["error"]["code"], json!(-32700));
    assert_eq!(response[0]["id"], Value::Null);
}

#[test]
fn a_one_megabyte_line_is_handled() {
    // 1 MiB of inline code is one very long line: the card would be far wider
    // than the size guard, so this fails without a browser.
    let huge = "a".repeat(1024 * 1024);
    let long_line = pipes(&[snapshot_call(json!(1), json!({ "code": huge }))]);
    let response = responses(&long_line);

    assert_eq!(response.len(), 1);
    assert_eq!(response[0]["result"]["isError"], json!(true));
    let text = response[0]["result"]["content"][0]["text"]
        .as_str()
        .expect("text");
    assert!(text.contains("too large"), "{text}");

    // A megabyte of garbage is a parse error, and the loop keeps going.
    let mut input = vec![b'x'; 1024 * 1024];
    input.extend_from_slice(b"\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"ping\"}\n");
    let response = responses(&run(&input));
    assert_eq!(response.len(), 2);
    assert_eq!(response[0]["error"]["code"], json!(-32700));
    assert_eq!(response[1]["id"], json!(2));
}

#[test]
fn deeply_nested_json_is_a_parse_error_and_the_loop_survives() {
    let mut nested = "[".repeat(200);
    nested.push('1');
    nested.push_str(&"]".repeat(200));
    let line = format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{nested}}}\n\
         {{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"ping\"}}\n"
    );

    let response = responses(&run(line.as_bytes()));
    assert_eq!(response.len(), 2);
    assert_eq!(response[0]["error"]["code"], json!(-32700));
    assert_eq!(response[0]["id"], Value::Null);
    assert_eq!(response[1]["id"], json!(2));
}

#[test]
fn duplicate_keys_take_the_last_value() {
    // serde_json keeps the last duplicate. Documented, not asserted as a spec:
    // JSON does not define duplicate keys and MCP clients do not send them.
    let session =
        run(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\",\"method\":\"tools/list\"}\n");
    let response = responses(&session);

    assert_eq!(response.len(), 1);
    assert!(
        response[0]["result"]["tools"].is_array(),
        "the last `method` wins: {}",
        response[0]
    );
}

#[test]
fn a_tools_call_meta_field_is_tolerated() {
    // MCP clients may attach `_meta` to request params. It is not an argument,
    // so it must not be reported as an unknown one.
    let response = responses(&pipes(&[request(
        json!(1),
        "tools/call",
        Some(json!({
            "name": "snapshot",
            "arguments": { "path": "definitely-not-here.rs" },
            "_meta": { "progressToken": "p1" }
        })),
    )]));

    assert_eq!(response.len(), 1);
    assert!(response[0].get("error").is_none(), "{}", response[0]);
    assert_eq!(response[0]["result"]["isError"], json!(true));
}

#[test]
fn the_tools_list_schema_is_structurally_valid() {
    let response = responses(&pipes(&[request(json!(1), "tools/list", None)]));
    let tools = response[0]["result"]["tools"].as_array().expect("tools");
    assert_eq!(tools.len(), 1);

    let tool = &tools[0];
    assert_eq!(tool["name"], "snapshot");
    let description = tool["description"].as_str().expect("description");
    assert!(description.contains("PNG"), "{description}");
    assert!(description.contains("`path`"), "{description}");
    assert!(description.contains("`code`"), "{description}");

    let schema = &tool["inputSchema"];
    assert_eq!(schema["type"], "object");
    assert_eq!(schema["additionalProperties"], json!(false));
    assert_eq!(schema["required"], json!([]));

    let properties = schema["properties"].as_object().expect("properties");
    let mut names: Vec<&str> = properties.keys().map(String::as_str).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        vec![
            "bg",
            "code",
            "font_size",
            "lang",
            "line_numbers",
            "lines",
            "out_dir",
            "padding",
            "path",
            "radius",
            "scale",
            "shadow",
            "theme",
            "title",
            "window",
            "write_files",
        ]
    );

    let allowed = ["string", "integer", "number", "boolean", "array", "object"];
    for (name, property) in properties {
        let kind = property["type"]
            .as_str()
            .unwrap_or_else(|| panic!("`{name}` has no string `type`: {property}"));
        assert!(
            allowed.contains(&kind),
            "`{name}` has an unknown type `{kind}`"
        );
        let text = property["description"]
            .as_str()
            .unwrap_or_else(|| panic!("`{name}` has no description"));
        assert!(!text.is_empty(), "`{name}` has an empty description");
        if kind == "integer" {
            assert!(
                property["minimum"].is_number(),
                "`{name}` declares `minimum`"
            );
            assert!(
                property["maximum"].is_number(),
                "`{name}` declares `maximum`"
            );
        }
        serde_json::to_string(property).expect("the property is serialisable");
    }
}

#[test]
fn every_schema_property_is_accepted_and_an_unknown_one_is_rejected() {
    // Each accepted case fails later, at the file read, so no browser is needed.
    let missing = "definitely-not-here.rs";
    let out_dir = std::env::temp_dir().to_string_lossy().to_string();
    let cases: Vec<(&str, Value)> = vec![
        ("lines", json!("2-3")),
        ("lang", json!("rust")),
        ("theme", json!("nord")),
        ("bg", json!("gradient")),
        ("title", json!("x.rs")),
        ("font_size", json!(15)),
        ("padding", json!(34)),
        ("radius", json!(14)),
        ("line_numbers", json!(false)),
        ("window", json!(false)),
        ("shadow", json!(false)),
        ("scale", json!(2)),
        ("out_dir", json!(out_dir)),
        ("write_files", json!(true)),
    ];

    let mut messages = Vec::new();
    let mut expected_ids = Vec::new();
    let mut id = 1i64;
    for (name, value) in &cases {
        let mut arguments = serde_json::Map::new();
        arguments.insert("path".to_string(), json!(missing));
        arguments.insert((*name).to_string(), value.clone());
        let mut arguments = Value::Object(arguments);
        if *name == "write_files" {
            arguments["out_dir"] = json!(out_dir);
        }
        messages.push(snapshot_call(json!(id), arguments));
        expected_ids.push(id);
        id += 1;
    }
    // `code` is exclusive with `path`; whitespace is accepted and then rejected
    // by the card builder ("the selection is empty"), still without a browser.
    messages.push(snapshot_call(json!(id), json!({ "code": "   " })));
    expected_ids.push(id);
    id += 1;
    // An undocumented argument is rejected by the schema.
    messages.push(snapshot_call(
        json!(id),
        json!({ "code": "x", "colour": "red" }),
    ));
    let unknown_id = id;

    let response = responses(&pipes(&messages));
    assert_eq!(response.len(), cases.len() + 2);

    for (index, id) in expected_ids.iter().enumerate() {
        assert_eq!(response[index]["id"], json!(id));
        assert!(
            response[index].get("error").is_none(),
            "case {} was accepted by the schema: {}",
            index,
            response[index]
        );
        assert_eq!(response[index]["result"]["isError"], json!(true));
    }

    let unknown = &response[cases.len() + 1];
    assert_eq!(unknown["id"], json!(unknown_id));
    assert_eq!(unknown["error"]["code"], json!(-32602));
}

#[test]
fn a_broken_browser_is_a_fast_tool_error_and_leaves_no_temp_directory() {
    let before = server_temp_dirs();

    let fixture = std::env::temp_dir().join(format!(
        "polacode-qa-not-a-browser-{}.txt",
        std::process::id()
    ));
    std::fs::write(&fixture, b"#!/bin/sh\necho nope\n").expect("write");

    let line = serde_json::to_string(&snapshot_call(json!(1), json!({ "code": "fn main() {}" })))
        .expect("encode");
    let session = run_with_env(
        format!("{line}\n").as_bytes(),
        &[("POLACODE_BROWSER", fixture.to_string_lossy().as_ref())],
    );
    let response = responses(&session);
    let _ = std::fs::remove_file(&fixture);

    assert_eq!(response.len(), 1);
    assert_eq!(response[0]["result"]["isError"], json!(true));
    let text = response[0]["result"]["content"][0]["text"]
        .as_str()
        .expect("text");
    assert!(
        text.contains(&fixture.to_string_lossy().to_string()),
        "the message names the executable it tried: {text}"
    );
    assert!(
        session.elapsed < Duration::from_secs(10),
        "a non-executable browser must fail fast, took {:?}",
        session.elapsed
    );

    let after = server_temp_dirs();
    assert!(
        after.is_subset(&before),
        "a failed render left a temp directory behind: {after:?}"
    );
}

#[test]
fn the_server_exits_zero_on_empty_and_closed_stdin() {
    let empty = run(b"");
    assert_eq!(empty.code, Some(0));
    assert!(empty.stdout.is_empty());
    assert!(empty.stderr.is_empty());

    // Stdin closed before a single byte is written: the same EOF path.
    let mut child = Command::new(BIN)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the server binary starts");
    drop(child.stdin.take());
    assert_eq!(wait_with_budget(&mut child), Some(0));
}

fn server_temp_dirs() -> std::collections::HashSet<std::path::PathBuf> {
    let mut found = std::collections::HashSet::new();
    let entries = match std::fs::read_dir(std::env::temp_dir()) {
        Ok(entries) => entries,
        Err(_) => return found,
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("zpolacode-mcp-") {
            found.insert(entry.path());
        }
    }
    found
}
