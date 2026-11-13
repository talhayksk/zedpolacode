//! `polacode-mcp` — an MCP stdio server that turns code into a PNG code card.
//!
//! stdout carries JSON-RPC and nothing else: one JSON object per line, flushed
//! after every message. Every diagnostic goes to stderr. A blank input line is
//! skipped, and EOF ends the process with exit code 0.

mod base64;
mod browser;
mod card;
mod protocol;
mod server;
mod tools;

#[cfg(test)]
mod fixture;

use std::io::{self, BufRead, Write};

use crate::browser::BrowserRenderer;
use crate::server::Server;

fn main() {
    let server = Server::new(BrowserRenderer::discover());

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(line) => line,
            Err(error) => {
                eprintln!("polacode-mcp: could not read stdin: {error}");
                break;
            }
        };

        if line.trim().is_empty() {
            continue;
        }

        let Some(response) = server.handle_line(&line) else {
            continue;
        };

        let text = match serde_json::to_string(&response) {
            Ok(text) => text,
            Err(error) => {
                eprintln!("polacode-mcp: could not encode a response: {error}");
                continue;
            }
        };

        if let Err(error) = write_message(&mut out, &text) {
            eprintln!("polacode-mcp: could not write to stdout: {error}");
            return;
        }
    }
}

fn write_message(out: &mut impl Write, text: &str) -> io::Result<()> {
    out.write_all(text.as_bytes())?;
    out.write_all(b"\n")?;
    out.flush()
}
