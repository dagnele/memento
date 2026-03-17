# AGENTS Guide

## Purpose

This file gives coding agents the minimum shared context needed to work safely and consistently in this repository.

The project is currently a small Rust CLI named `memento`.
It uses a command-oriented architecture with a local server, service, and render split.

## Repository Snapshot

- Language: Rust
- Crate type: single binary crate
- Manifest: `Cargo.toml`
- Entry point: `src/main.rs`
- CLI parsing: `clap`
- Error handling: `anyhow`
- Product behavior is defined by the current code and tests in this repository.

## External Instruction Files

Checked for repository-level editor/agent rules:

- `.cursorrules`: not present
- `.cursor/rules/`: not present
- `.github/copilot-instructions.md`: not present

If any of those files are added later, update this document to reflect them.

## Core Product Direction

Follow the current CLI behavior and tests when implementing behavior.

Important product constraints from the current code and tests:

- Fully local by default
- Fast startup and simple execution model
- Single binary CLI in Rust
- Minimal command surface
- Filesystem-first mental model
- No remote or cloud APIs in the MVP
- Local loopback server mode is part of the current design

## Current CLI Surface

The current commands are:

- `memento init [--model <model>] [--port <port>]`
- `memento serve`
- `memento doctor`
- `memento models`
- `memento add [--force] <path>...`
- `memento rm <path|uri>`
- `memento remember --namespace <user|agent> --path <path> (<text> | --file <source>)`
- `memento forget <uri>`
- `memento reindex [path]...`
- `memento ls [uri]`
- `memento cat <uri>`
- `memento show <uri>`
- `memento find <query>`

## Build Commands

Run from the repository root `C:\src\memento.git`.

- Build debug binary: `cargo build`
- Build release binary: `cargo build --release`
- Run the CLI: `cargo run -- <subcommand>`
- Check compilation without producing a final binary: `cargo check`

Useful examples:

- `cargo run -- init`
- `cargo run -- add README.md src`
- `cargo run -- ls`
- `cargo run -- show mem://resources/project/src/main.rs`
- `cargo run -- find "auth flow"`

## Formatting Commands

- Format the crate: `cargo fmt`
- Check formatting without modifying files: `cargo fmt --check`

Formatting should be treated as required for Rust code changes.

## Lint Commands

- Run Clippy: `cargo clippy --all-targets --all-features`
- Treat Clippy warnings as errors when tightening quality gates: `cargo clippy --all-targets --all-features -- -D warnings`

Because this is a single-crate repository, these commands are sufficient today.

## Test Commands

- Run all tests: `cargo test`
- Run tests and show stdout: `cargo test -- --nocapture`
- List available tests: `cargo test -- --list`

### Running a Single Test

Use Cargo's test filter:

- Run one test by substring: `cargo test test_name`
- Run one exact test: `cargo test test_name -- --exact`

For the current integration test target under `tests/`, you can also run:

- `cargo test --test cli_flow`
- `cargo test --test cli_flow single_case -- --exact`

## Current Code Layout

- `src/main.rs`: program entry point
- `src/bootstrap.rs`: workspace initialization logic
- `src/cli.rs`: Clap parser and subcommand definitions
- `src/client.rs`: client transport for server-routed commands
- `src/config.rs`: workspace config load, validation, and persistence
- `src/server.rs`: local server entry point
- `src/dispatch.rs`: request dispatch for server-routed commands
- `src/protocol.rs`: request and response protocol types
- `src/embedding.rs`: embedding model selection and cache helpers
- `src/indexing.rs`: text segmentation and indexing helpers
- `src/repository/*.rs`: SQLite-backed workspace repository
- `src/resource_state.rs`: tracked resource state inspection
- `src/service/*.rs`: one file per command/service implementation
- `src/render/*.rs`: CLI output rendering
- `src/text_file.rs`: text-file validation and loading helpers
- `src/timing.rs`: timing helpers for user-facing output
- `src/uri.rs`: `mem://` URI parsing and formatting
- `tests/cli_flow.rs`: end-to-end CLI integration coverage

## Architectural Rules

Keep the command-pattern structure intact unless there is a compelling reason to change it.

Preferred flow:

1. Parse CLI input in `src/cli.rs`
2. Handle `init` in `src/bootstrap.rs`, `serve` in `src/server.rs`, and route other commands through `src/protocol.rs` and `src/client.rs`
3. Execute server-routed behavior through `src/dispatch.rs` and `src/service/*.rs`
4. Render structured results in `src/render/*.rs`

For new subcommands:

- add the variant to `CliCommand`
- add the matching remote protocol shape when needed
- wire it into dispatch and rendering

## Style Guidelines

Use standard Rust idioms first. Prefer boring, obvious code over clever abstractions.

### Rust Design Guidance

The repository should follow the spirit of SOLID, but adapted to idiomatic Rust rather than object-oriented design.

- Single Responsibility: keep modules, structs, and functions focused on one clear job
- Open/Closed: prefer extension through new command types, helper functions, and small modules instead of editing unrelated code paths
- Liskov Substitution: when using traits, keep behavior contracts simple and predictable so implementations can be swapped safely
- Interface Segregation: prefer small traits with narrow responsibilities over large capability traits
- Dependency Inversion: depend on stable abstractions only when there is a real boundary, such as command execution, storage, or model backends

Rust-specific guidance:

- Prefer composition over inheritance; use structs and helper functions instead of deep abstraction trees
- Prefer enums for closed sets of behavior and traits for open extension points
- Do not introduce traits just to satisfy a pattern; add them only when multiple implementations or test seams are needed
- Keep ownership and lifetimes simple; clone small values when it keeps code clearer
- Favor explicit data flow and explicit state over hidden mutation
- Prefer pure helpers for parsing, normalization, and transformation logic when practical

### Formatting

- Always use `cargo fmt`
- Keep lines and layout rustfmt-friendly
- Do not manually fight rustfmt output

### Imports

- Group imports by standard library, external crates, then local crate imports when practical
- Prefer explicit imports over glob imports
- Remove unused imports promptly
- Keep import ordering consistent with rustfmt defaults

### Types

- Use concrete types unless abstraction clearly improves the design
- Prefer `String` for owned CLI/user input and `&str` for borrowed views
- Prefer small structs and enums with clear responsibilities
- Introduce traits only when they support an actual boundary, such as storage or embedding seams

### Naming

- Types and traits: `UpperCamelCase`
- Functions, modules, and variables: `snake_case`
- Constants: `SCREAMING_SNAKE_CASE`
- Result and protocol types should use descriptive names like `FindResult` and `ExecuteRequest`
- Avoid abbreviations unless they are standard in Rust or the CLI domain

### Error Handling

- Use `anyhow::Result` for application-level fallible functions
- Prefer returning errors over panicking
- Reserve `panic!` and `unwrap()` for cases that are truly impossible or for test code
- Add context to errors when interacting with the filesystem, parsing, or storage layers
- Surface user-facing errors clearly from command execution paths

### Control Flow

- Prefer early returns over deep nesting
- Keep command `execute` methods easy to scan
- Extract helpers when a function starts doing more than one clear job

### Comments and Docs

- Add comments only when the intent is not obvious from the code
- Prefer expressive naming over explanatory comments
- Keep doc comments focused on public behavior or invariants

### CLI Output

- Keep output concise and user-facing
- Match the wording already used by the CLI and tests when practical
- Avoid noisy debug printing in committed code

## Implementation Expectations

When changing behavior, prefer small vertical slices:

- keep changes scoped to one command or subsystem when practical
- keep the CLI interface stable
- update repository documentation when behavior changes materially

## Filesystem and Workspace Notes

The intended workspace root is `.memento/` in the current repository.

- `.memento/config.toml`
- `.memento/index.db`
- `.memento/user/`
- `.memento/agent/`

Do not introduce remote dependencies or non-local services unless the project direction explicitly changes.

## Agent Editing Guidance

- Preserve the minimal API unless the task explicitly changes the product direction
- Do not add remote ingestion flows, cloud integrations, or extra namespaces unless requested
- Prefer extending existing files and patterns over introducing large frameworks
- Keep changes small, reviewable, and consistent with the MVP direction

## Before Finishing a Code Change

At minimum, run:

- `cargo fmt --check`
- `cargo build`

When tests exist, also run:

- `cargo test`

When linting matters for the change, also run:

- `cargo clippy --all-targets --all-features -- -D warnings`

## If You Need More Context

Start with:

- `src/cli.rs`
- `src/protocol.rs`
- `src/dispatch.rs`
- `tests/cli_flow.rs`

Those files define the current interface and behavior of the project.
