# AGENTS Guide

Small Rust CLI named `memento`. Single binary crate, `clap` for CLI parsing, `anyhow` for errors.

## Product Constraints

- Fully local by default, no remote or cloud APIs
- Single binary, fast startup, minimal command surface
- Filesystem-first mental model
- Local loopback server mode is part of the design

## Build / Test / Lint

```
cargo build
cargo test
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
```

Run at minimum `cargo fmt --check && cargo build` before finishing a change. Run `cargo test` when tests exist for the area touched.

## Code Layout

- `src/main.rs` — entry point
- `src/cli.rs` — Clap parser and subcommand definitions
- `src/bootstrap.rs` — workspace initialization (`init`)
- `src/server.rs` — local server (`serve`)
- `src/client.rs` — client transport for server-routed commands
- `src/config.rs` — workspace config
- `src/dispatch.rs` — request dispatch for server-routed commands
- `src/protocol.rs` — request/response types
- `src/embedding.rs` — embedding model selection and cache
- `src/indexing.rs` — text segmentation and indexing
- `src/repository/*.rs` — SQLite-backed workspace repository
- `src/resource_state.rs` — tracked resource state
- `src/service/*.rs` — one file per command implementation
- `src/render/*.rs` — CLI output rendering
- `src/text_file.rs` — text-file validation and loading
- `src/timing.rs` — timing helpers
- `src/uri.rs` — `mem://` URI parsing and formatting
- `tests/cli_flow.rs` — end-to-end integration tests

## Architecture

Command flow: CLI parse (`cli.rs`) -> `init` goes to `bootstrap.rs`, `serve` to `server.rs`, other commands route through `protocol.rs`/`client.rs` -> `dispatch.rs` -> `service/*.rs` -> `render/*.rs`.

For new subcommands: add variant to `CliCommand`, add protocol shape if needed, wire into dispatch and rendering.

## Workspace

Root is `.memento/` containing `config.toml`, `index.db`, `user/`, `agent/`.

## Key Context Files

Start with `src/cli.rs`, `src/protocol.rs`, `src/dispatch.rs`, `tests/cli_flow.rs` when you need to understand the interface and behavior.
