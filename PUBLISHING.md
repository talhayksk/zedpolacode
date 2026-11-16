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
| ID unique, kebab-cased, no `zed`/`extension` words | ✅ | `id = "zpolacode"` in `extension.toml`. It does not contain the word `zed`, and the ID is free in the registry (1590 entries checked). The repo and crate stay `zedpolacode`. |
| All user-facing text in English | ✅ | Assistant output, `--help`, errors and descriptors are English. |
| Allowed license | ✅ | MIT. |
| Test at the exact submodule commit | ✅ | `cargo test` + a manual run in Zed before tagging. |
| Do not read/modify outside the environment Zed designates | ⚠️ | Writing the PNG needs a host process (Node + headless browser), because the WASI sandbox only allows writes into the extension work dir. This is exactly the kind of workaround the docs say is only accepted "at the maintainers' discretion". |
| Do not misuse the extension API to work around limitations | ⚠️ | Same as above. |
| Publish functionality not already in the registry | ✅ | No `zpolacode` entry in `extensions.toml`. |

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
3. Keep a thin extension (`id = "zpolacode-mcp-server"`) that only locates or
   downloads that server. Note that MCP extensions have their own ID rule
   (prefix `mcp-server-` or suffix `-mcp-server`), so this ID will not be
   `zpolacode`.

Note the prerequisite: *"Only provide one MCP server and nothing else."* The
slash command would have to go.

### 2. Ship it as an MCP server only

Skip Zed extensions entirely and publish the snapshot tool as an MCP server
(configurable in Zed through `context_servers`). Same code, no registry review.

Both options keep the current rendering engine; only the entry point changes.

## Publishing to the MCP registry

The server is packaged for the official [MCP registry](https://registry.modelcontextprotocol.io/)
by `npm/zpolacode-mcp/server.json`. That file does not stand alone: the registry
fetches the npm package named in it and checks the `mcpName` and version it
finds there, so **publish npm first, then the registry entry**.

1. Bump the version in three places: `crates/mcp/Cargo.toml`, then
   `npm/zpolacode-mcp/package.json` and `npm/zpolacode-mcp/server.json`. The npm
   test suite fails if the last two disagree, so `npm test` is part of the
   pre-flight check. Tag `mcp-v<version>`; `release-mcp.yml` rejects a tag that
   disagrees with the crate and builds the five platform archives the npm
   wrapper downloads.
2. Publish the wrapper once the release exists:

   ```bash
   cd npm/zpolacode-mcp
   npm publish
   ```

3. Publish the registry entry. Install `mcp-publisher` from its GitHub releases,
   then authenticate and publish from the directory holding `server.json`:

   ```bash
   cd npm/zpolacode-mcp
   mcp-publisher login github
   mcp-publisher publish
   ```

   `mcp-publisher login github` proves ownership of the `io.github.talhayksk/*`
   namespace through GitHub, which is why the name in `server.json` must stay
   under that namespace. Registry names are immutable, so the 0.3.0 rename from
   `polacode` to `zpolacode` publishes a new entry; mark the old one deprecated
   rather than trying to edit it.

`server.json` is validated in CI (`npm/zpolacode-mcp/test/server.test.js`), so a
version bump that forgets one of the two files fails before a publish attempt.

## Signing the release binaries

SmartScreen and "virus" warnings are about the **downloaded executable**, not
the extension: Zed compiles `extension.wasm` locally from source, so there is no
prebuilt Zed artifact to sign. The warnings appear on `zpolacode-mcp.exe` (and on
macOS) because it is an unsigned binary with no download reputation. Code
signing does not change the code, it adds a publisher identity and a timestamp.

Signing is **optional and secret-driven**: every signing step in
`release-mcp.yml` is skipped when its secret is missing, so a release without
certificates still succeeds — it is just annotated as unsigned.

### Windows — Authenticode

Get a code-signing certificate (OV or EV from a CA, or Azure Trusted Signing)
with an exportable private key, then add two repository secrets:

| Secret | Value |
| --- | --- |
| `WINDOWS_CERT_PFX_BASE64` | `base64 -w0 cert.pfx` (PowerShell: `[Convert]::ToBase64String([IO.File]::ReadAllBytes('cert.pfx'))`) |
| `WINDOWS_CERT_PASSWORD` | the PFX password |

The workflow signs `zpolacode-mcp.exe` with SHA-256 and a DigiCert timestamp
**before** zipping, then verifies the result with `Get-AuthenticodeSignature`; a
signature that does not verify fails the release instead of shipping silently.

Reputation matters as much as the signature: with an OV certificate SmartScreen
warnings fade as downloads accrue, while EV certificates and Trusted Signing
start trusted. If Defender still heuristically flags a signed build, submit the
archive at <https://www.microsoft.com/en-us/wdsi/filesubmission> and explain
that it is a Rust MCP server released by the repository.

### macOS — Developer ID and notarisation

Requires Apple Developer Program membership and a "Developer ID Application"
certificate exported as `.p12`:

| Secret | Value |
| --- | --- |
| `MACOS_CERT_P12_BASE64` | `base64 -w0 cert.p12` |
| `MACOS_CERT_PASSWORD` | the `.p12` password |
| `MACOS_SIGNING_IDENTITY` | e.g. `Developer ID Application: Talha Yüksek (TEAMID)` |
| `APPLE_ID` | the Apple ID that owns the team |
| `APPLE_TEAM_ID` | the ten-character team ID |
| `APPLE_APP_PASSWORD` | an app-specific password from <https://appleid.apple.com> |

The workflow imports the certificate into a throw-away keychain, signs with the
hardened runtime, zips, and submits the zip to `notarytool --wait`. A rejected
notarisation fails the job. The notary ticket lives on Apple's servers (a bare
Mach-O cannot be stapled), so the first launch on a machine that has never seen
the binary needs network access to fetch it.

Without these secrets the macOS build stays unsigned and Gatekeeper will warn on
first launch.

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
   [zpolacode]
   submodule = "extensions/zedpolacode"
   version = "0.1.3"
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
