# zpolacode-mcp

Turn code into a shareable **PNG code card** from the **Zed Polacode** extension, and
run the same engine as an **MCP stdio server** — from one npm package.

This package is a **distribution wrapper**. It contains no rendering code: on install (or on
first run) it downloads the real `zpolacode-mcp` binary for your platform from the project's
GitHub Releases, caches it, and then runs it. With no arguments that binary is the MCP stdio
server; with `card` it is a small command-line tool. (The package was called
`polacode-mcp` before 0.3.0.)

```bash
npx -y zpolacode-mcp                 # MCP stdio server (this is the whole point)
npx -y zpolacode-mcp --version       # zpolacode-mcp 0.3.0
npx -y zpolacode-mcp card --clipboard --lang Rust --title demo.rs --open
```

Prefer a permanent command? `npm install -g zpolacode-mcp` puts `zpolacode-mcp` on your `PATH`,
and the MCP client configs below can then use `"command": "zpolacode-mcp"` with no `args`.

## What you get

| Surface | Command | Notes |
| --- | --- | --- |
| MCP server | `zpolacode-mcp` | stdio transport, one tool: `snapshot` |
| Card CLI | `zpolacode-mcp card …` | renders the same card with no model in the loop |

The server exposes one tool, `snapshot`, which turns a file and a line range into a PNG card
(plus the HTML it was rendered from) and returns its path and size.

## MCP clients

### Claude Desktop

`claude_desktop_config.json` (`claude.ai/download` → *Settings → Developer → Edit Config`):

```json
{
  "mcpServers": {
    "zpolacode": {
      "command": "npx",
      "args": ["-y", "zpolacode-mcp"]
    }
  }
}
```

Restart the client, and the `snapshot` tool is available. Any other MCP client that can start
a stdio process takes the same two fields.

### Zed

In the Zed settings file (`zed: open settings file`):

```json
{
  "context_servers": {
    "zpolacode": {
      "source": "custom",
      "command": "npx",
      "args": ["-y", "zpolacode-mcp"],
      "env": {}
    }
  }
}
```

Then **Settings → AI → MCP Servers** should show `zpolacode` as active. `env` is the place to
put `POLACODE_BROWSER` if browser auto-discovery picks the wrong one.

### opencode

In `~/.config/opencode/opencode.json` (global) or `opencode.json` in the project root:

```json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "zpolacode": {
      "type": "local",
      "command": ["npx", "-y", "zpolacode-mcp"],
      "enabled": true
    }
  }
}
```

`command` is an argument array, not a shell string. Add
`"environment": { "POLACODE_BROWSER": "<path>" }` to the same object if browser
auto-discovery picks the wrong one. Restart opencode to pick up the server.

### Claude Code

```bash
claude mcp add zpolacode -- npx -y zpolacode-mcp
```

Relative `path` and `out_dir` arguments are resolved against the server's working directory,
which the client chooses. Pass absolute paths unless you know what that directory is.

## Card CLI

```text
zpolacode-mcp card --clipboard [options]
zpolacode-mcp card --file <path> [--lines <range>] [options]
```

Exactly one source flag is required.

```bash
# selected code is already on the clipboard (Ctrl+C first)
npx -y zpolacode-mcp card --clipboard --lang Rust --title demo.rs --open

# a file, lines 40-70 of it
npx -y zpolacode-mcp card --file src/main.rs --lines 40-70 --open

# a JSON summary instead of the human line, written somewhere else
npx -y zpolacode-mcp card --file src/main.rs --lines 1-20 --out ./cards --json
```

| Flag | Value | Default |
| --- | --- | --- |
| `--clipboard` / `--file <path>` | source, exactly one | — |
| `--lines <range>` | `40-70`, `40:70`, `40..70`, `40+70`, `40`, `40-` (file mode only) | whole file |
| `--lang <id>` | language id or Zed language name (`Rust`, `Shell Script`, …) | file extension, then `plaintext` |
| `--title <text>` | title bar text | file name, or `snippet` |
| `--theme <id>` | `dracula`, `one-dark`, `nord`, `tokyo-night`, `github-dark`, `catppuccin-mocha`, `github-light`, `solarized-light` | `dracula` |
| `--bg <name\|#hex>` | `transparent`, `dark`, `light`, `gradient`, `sunset`, `ocean`, `aurora`, `midnight`, `mono`, or a hex colour | `transparent` |
| `--font-size` / `--padding` / `--radius` / `--scale` | `9-40` / `0-120` / `0-48` / `1-4` | `15` / `34` / `14` / `2` |
| `--no-line-numbers` / `--no-window` / `--no-shadow` | flags | line numbers off for `--clipboard` |
| `--out <dir>` | output directory | `.zedpolacode` in the working directory |
| `--open` | open the PNG in the platform viewer | off |
| `--json` | one JSON object instead of the human line | off |

Exit codes: `0` success, `2` bad arguments, `1` runtime failure (empty clipboard, unreadable
file, no browser, unwritable output). `zpolacode-mcp card --help` prints the full usage.

Output is written as `<out>/<base>.png` and `<out>/<base>.html`. With `--open` the viewer is
handed a throw-away copy in the system temp directory, so the project PNG is always writable
and always holds the newest card.

## Requirements

- **Node.js 18 or newer** — this wrapper is Node only.
- **A browser for PNG output**: Chrome, Edge, Chromium or Brave. Without one the CLI and the
  MCP tool still produce the HTML card and explain why there is no PNG. Set
  `POLACODE_BROWSER` (or `CHROME_PATH`) to a browser executable to choose one explicitly.

## Supported platforms

| Platform | Release triple |
| --- | --- |
| Windows x64 | `x86_64-pc-windows-msvc` |
| macOS x64 (Intel) | `x86_64-apple-darwin` |
| macOS arm64 (Apple Silicon) | `aarch64-apple-darwin` |
| Linux x64 | `x86_64-unknown-linux-gnu` |
| Linux arm64 | `aarch64-unknown-linux-gnu` |

Anything else fails immediately with a message naming the pair and this list; there is no
fallback to another architecture. The Linux binaries link against glibc 2.35 (Ubuntu 22.04),
so older distributions (RHEL 8, for example) cannot run them.

The macOS binaries are signed with a Developer ID certificate and notarised when the
release was built with Apple credentials, and unsigned otherwise; a download by this
wrapper does not set the quarantine attribute, but a binary fetched by a browser may be
blocked by Gatekeeper when the release is unsigned.

## Where the binary lives

```text
<os.homedir()>/.zpolacode-mcp/<version>/<triple>/zpolacode-mcp[.exe]
```

The version directory is why upgrading never reuses an old binary. To move the cache — a
read-only home directory, a container, a test — set `ZPOLACODE_MCP_CACHE_DIR`; it replaces
`<os.homedir()>/.zpolacode-mcp` and the `<version>/<triple>` subdirectories are still used:

```bash
ZPOLACODE_MCP_CACHE_DIR=/opt/zpolacode-cache npx -y zpolacode-mcp --version
```

Removing a version's directory forces a fresh download the next time.

### Install time vs first run

`postinstall` downloads the binary so the first `npx` is instant. If that download fails —
no network, a proxy, a read-only home directory — the install still succeeds, prints one
line, and the binary is fetched on the first run instead. Installs therefore never fail
because GitHub was unreachable.

On Windows, security software can refuse to start a program while it scans the file that was
just written. The wrapper retries briefly and then says so; running the command again a moment
later starts the server. Official releases are Authenticode-signed when the release pipeline
has a certificate configured; an unsigned build can raise a SmartScreen "unknown publisher"
prompt. See `PUBLISHING.md` in the repository for the signing setup and for verifying a
download against the release's `sha256sums.txt`.

If you would rather not run the postinstall download at all, install with
`npm install --ignore-scripts zpolacode-mcp` (or `npm_config_ignore_scripts=true`); the bin
downloads on demand.

### Advanced: point at a binary yourself

`ZPOLACODE_MCP_BINARY` is an escape hatch for local development, a system package, or a
network-isolated machine. It skips the cache and the download entirely:

```bash
ZPOLACODE_MCP_BINARY=/usr/local/bin/zpolacode-mcp npx -y zpolacode-mcp --version
```

The path must be a native executable, or a Node script (`.js`, `.cjs`, `.mjs`), which is run
with the same Node that runs the wrapper. A non-Node script without an executable format
cannot be started on Windows.

## Versioning

The npm version tracks the server release it installs, one to one:

- npm `0.3.0` downloads
  `https://github.com/talhayksk/zedpolacode/releases/download/mcp-v0.3.0/zpolacode-mcp-<triple>.zip`
- `zpolacode-mcp --version` prints the same number as the MCP server's `serverInfo.version`.

Releases before 0.3.0 shipped as `polacode-mcp`; their archives keep the old
`polacode-mcp-<triple>.zip` names, and the npm package of that name stops at `0.2.1`.

A new server release therefore means a new npm version: patch for fixes, minor for new tool
parameters or flags, major for a breaking tool schema. The package itself contains no logic
that would need its own version train.

Downloads are plain HTTPS from GitHub Releases; no checksum file is consulted by the wrapper.
The extraction is a small in-package zip reader: the binary at the archive root, stored or
deflate, no zip64. Anything else (an HTML error page, a truncated download, an unexpected
archive layout) is rejected with a named reason rather than half-installed, and the binary is
written through a temporary file and renamed, so a killed install cannot leave a truncated
executable behind.

## License

[MIT](LICENSE) © Talha Yüksek. The binary this package downloads is MIT too; the source lives
at <https://github.com/talhayksk/zedpolacode>.
