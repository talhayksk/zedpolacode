# Changelog

This project follows [Semantic Versioning](https://semver.org/).

## Unreleased

- The MCP server is now `zpolacode-mcp`: crate, binary, npm package, release
  archives, the `~/.zpolacode-mcp` cache and the `ZPOLACODE_MCP_*` variables all
  carry the `z` prefix so the name says it belongs to Zed Polacode. The registry
  entry is `io.github.talhayksk/zpolacode`, replacing `polacode`.
- `release-mcp.yml` can sign the release binaries: Authenticode on Windows and
  Developer ID plus notarisation on macOS. The steps activate only when the
  signing secrets exist, so releases still build without certificates.

## polacode-mcp 0.2.1 — 2026-11-16

- `card --open` shows a throw-away copy in the system temp directory, so a
  second snapshot never collides with an image viewer still holding the first.
- Distributed on npm as `polacode-mcp`, a dependency-free wrapper that
  downloads the matching release binary and forwards stdio, arguments, exit
  codes and signals; `npx -y polacode-mcp` is the server.
- `npm/polacode-mcp/server.json` describes the server for the official MCP
  registry, and a test keeps it in step with `package.json`.

## polacode-mcp 0.2.0 — 2026-11-16

- `card` CLI mode: `--clipboard` or `--file`, the same card pipeline without a
  model in the loop, with `--json`, `--out` and `--open`.

## polacode-mcp 0.1.0 — 2026-11-16

- MCP stdio server exposing one `snapshot` tool (file path or inline code).
- Release pipeline producing five platform zips plus `sha256sums.txt`.

## 0.1.3 — 2026-11-12

- Registry ID is now `zpolacode`: the rule forbids the *word* `zed`, not the
  letter `z`, so the product name survives in a compliant form.
- Split the engine into a `polacode-core` crate (`crates/core`): highlighting,
  themes, card rendering and a front-end agnostic `CardOptions`. The Zed
  extension is now a thin wrapper around it, which is the first step towards
  sharing the same engine with an MCP server.

## 0.1.2 — 2026-11-12

- The slash command is now `/zedpolacode`, matching the product name.
- The default output folder is `.zedpolacode/` (previously `.polacode/`).
- README rewritten around the Zed Polacode name, with a note that the registry
  ID stays `polacode` because extension IDs may not contain `zed`.

## 0.1.1 — 2026-11-12

Aligns the extension with the current Zed extension docs.

- Build with `wasm32-wasip2` (Zed's target for extensions) instead of
  `wasm32-wasip1`; CI and docs updated.
- Registry ID is now `polacode`: the publishing prerequisites require IDs to be
  kebab-cased and to not contain `zed` (repo and crate stay `zedpolacode`).
- All user-facing text (help, assistant output, errors, descriptors) is English.
- Documented that slash-command extensions are no longer accepted in the Zed
  registry, and what the alternatives are (see `PUBLISHING.md`).

## 0.1.0 — 2026-11-12

Initial release.

- `/polacode` slash command: build a code card from a file or a line range.
- Options: `--lines`, `--theme`, `--bg`, `--title`, `--lang`, `--font-size`,
  `--padding`, `--radius`, `--no-line-numbers`, `--no-window`, `--no-shadow`,
  `--out`, `--format`, plus slash-command completions.
- 8 themes: dracula, one-dark, nord, tokyo-night, github-dark, catppuccin-mocha,
  github-light, solarized-light.
- Dependency-free highlighting for 23 languages (with dedicated rules for
  HTML, CSS, JSON, YAML and TOML).
- Output: `@2x` PNG (via Chrome/Edge/Chromium/Brave) plus a self-contained HTML
  card. Transparent background, gradients and hex colors.
- If the PNG cannot be rendered the command still succeeds with the HTML card.
- The card is produced inside the extension work dir (WASI sandbox) and handed
  to a host-side Node script that writes it into the project.
