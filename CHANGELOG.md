# Changelog

This project follows [Semantic Versioning](https://semver.org/).

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
