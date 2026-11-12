# Publishing notes

Everything below is based on the official Zed docs:

- [Developing Extensions](https://zed.dev/docs/extensions/developing-extensions)
- [Extension Publishing Prerequisites](https://zed.dev/docs/extensions/publishing/prerequisites)
- [Publishing guide](https://zed.dev/docs/extensions/publishing/publishing-guide)

## TL;DR

**A slash-command-only extension cannot be submitted to the Zed extension
registry today.** The prerequisites page states:

> ### Agent Server and Slash Command Extensions
>
> Agent server and slash command extensions have been deprecated and submissions
> will no longer be accepted. If you intend on making an agent server available
> within Zed, publish it to the [ACP Registry](https://agentclientprotocol.com/registry)
> instead.

So this project is distributed as a **dev extension** (clone the repo, run
`zed: install dev extension`). That is a supported, documented way to use an
unpublished extension in Zed.

## How the published rules map to this extension

| Prerequisite (docs) | Status here | Notes |
| --- | --- | --- |
| Slash command extensions accepted | ❌ | Deprecated; submissions rejected. |
| ID unique, kebab-cased, no `zed`/`extension` words | ✅ | `id = "polacode"` in `extension.toml`. The repo/crate stay `zedpolacode`, only the registry ID must be neutral. |
| All user-facing text in English | ✅ | Assistant output, `--help`, errors and descriptors are English. |
| Allowed license | ✅ | MIT. |
| Test at the exact submodule commit | ✅ | `cargo test` + a manual run in Zed before tagging. |
| Do not read/modify outside the environment Zed designates | ⚠️ | Writing the PNG needs a host process (Node + headless browser), because the WASI sandbox only allows writes into the extension work dir. This is exactly the kind of workaround the docs say is only accepted "at the maintainers' discretion". |
| Do not misuse the extension API to work around limitations | ⚠️ | Same as above. |
| Publish functionality not already in the registry | ✅ | No `polacode` entry in `extensions.toml`. |

## If you want registry distribution anyway

Two realistic routes, both bigger than "change the ID":

### 1. Pivot to an MCP server extension

The docs still accept MCP server extensions (with `mcp-server-` prefix or
`-mcp-server` suffix, one server per extension, server **not** bundled — it must
be downloaded or found in the environment). That would mean:

1. Move the card renderer (highlight + HTML + headless screenshot) into a
   standalone Node/Python MCP server published on npm/PyPI.
2. Publish it to the [MCP registry](https://registry.modelcontextprotocol.io/)
   as well, since MCP *extensions* are themselves being deprecated in favour of
   the registry.
3. Keep a thin extension (`id = "polacode-mcp-server"`) that only locates or
   downloads that server.

Note the prerequisite: *"Only provide one MCP server and nothing else."* The
slash command would have to go.

### 2. Ship it as an MCP server only

Skip Zed extensions entirely and publish the snapshot tool as an MCP server
(configurable in Zed through `context_servers`). Same code, no registry review.

Both options keep the current rendering engine; only the entry point changes.

## Registry submission steps (kept for reference)

These steps apply to any *accepted* extension category, in case this project is
ever restructured into one:

1. Fork [zed-industries/extensions](https://github.com/zed-industries/extensions).
2. Clone your fork and add the submodule:
   ```bash
   git submodule add https://github.com/talhayksk/zedpolacode.git extensions/zedpolacode
   ```
3. Add the entry to `extensions.toml`:
   ```toml
   [polacode]
   submodule = "extensions/zedpolacode"
   version = "0.1.1"
   ```
4. Commit, push, and open a PR against `zed-industries/extensions`.
5. CI in that repository validates the manifest, the license, and that the
   extension builds for `wasm32-wasip2`.

### Releasing a new version

1. Bump `version` in `extension.toml`, `Cargo.toml` and `CHANGELOG.md`.
2. `git tag -a vX.Y.Z -m "..."` and `git push --tags`.
3. Update the submodule commit and `version` in the registry fork, then open a PR.

## Pre-release checklist

- [ ] `cargo fmt --check` clean
- [ ] `cargo test` green
- [ ] `cargo build --release --target wasm32-wasip2` succeeds
- [ ] `extension.toml` has `id`, `name`, `version`, `schema_version`, `authors`,
      `description`, `repository`
- [ ] README explains install and usage
- [ ] `LICENSE` present
- [ ] PNG output verified by hand in Zed (Chrome/Edge installed)
