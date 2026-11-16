//! `zpolacode-mcp` — an MCP stdio server that turns code into a PNG code card.
//!
//! With no arguments it is the stdio server: stdout carries JSON-RPC and nothing
//! else, one JSON object per line, flushed after every message, and every
//! diagnostic goes to stderr. A blank input line is skipped, and EOF ends the
//! process with exit code 0.
//!
//! With arguments it is the `card` CLI (see [`cli`]): the same binary and the
//! same card pipeline, driven from the command line or a Zed keybinding instead
//! of a model.

mod base64;
mod browser;
mod card;
mod cli;
mod protocol;
mod server;
mod tools;

#[cfg(test)]
mod fixture;

use std::io::{self, BufRead, Write};

use crate::browser::BrowserRenderer;
use crate::server::Server;

fn main() {
    let args = match collect_arguments() {
        Ok(args) => args,
        Err(message) => {
            eprintln!("zpolacode-mcp: {message}");
            std::process::exit(cli::EXIT_USAGE);
        }
    };

    // No arguments is the MCP server, byte for byte the old entry point.
    if !args.is_empty() {
        let code = cli::main(&args);
        let _ = io::stdout().flush();
        std::process::exit(code);
    }

    run_server();
}

/// Everything after the program name. `args_os` avoids the panic `args` has on
/// an argument that is not valid Unicode.
fn collect_arguments() -> Result<Vec<String>, String> {
    let mut args = Vec::new();
    for (index, argument) in std::env::args_os().skip(1).enumerate() {
        match argument.into_string() {
            Ok(text) => args.push(text),
            Err(_) => {
                return Err(format!(
                    "argument {} is not valid UTF-8; pass UTF-8 text.",
                    index + 1
                ))
            }
        }
    }
    Ok(args)
}

fn run_server() {
    let server = Server::new(BrowserRenderer::discover());

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(line) => line,
            Err(error) => {
                eprintln!("zpolacode-mcp: could not read stdin: {error}");
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
                eprintln!("zpolacode-mcp: could not encode a response: {error}");
                continue;
            }
        };

        if let Err(error) = write_message(&mut out, &text) {
            eprintln!("zpolacode-mcp: could not write to stdout: {error}");
            return;
        }
    }
}

fn write_message(out: &mut impl Write, text: &str) -> io::Result<()> {
    out.write_all(text.as_bytes())?;
    out.write_all(b"\n")?;
    out.flush()
}
