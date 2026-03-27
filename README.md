# memento

`memento` is a fast, local Rust CLI for indexing project files and personal workspace memory, then retrieving relevant context with semantic search.

It is inspired by [OpenViking](https://github.com/volcengine/OpenViking), with a narrower CLI-first workflow focused on local files, local memory, and a single-user developer setup.

## What it does

- Index local text files into a workspace-managed vector store
- Store user and agent notes under stable `mem://` URIs
- Search indexed content with local embedding models
- Inspect, list, read, reindex, and remove tracked items
- Run entirely on your machine with a local loopback server

## Current command surface

```text
memento init [--model <model>] [--port <port>]
memento serve
memento doctor
memento models
memento add [--force] <path>...
memento rm <path|uri>
memento remember --namespace <user|agent> --path <path> (<text> | --file <source>)
memento forget <uri>
memento reindex [path]...
memento ls [uri]
memento cat <uri>
memento show <uri>
memento find <query>
```

## Installation

### Linux / macOS

```bash
curl -LsSf https://raw.githubusercontent.com/dagnele/memento/main/install.sh | bash
```

### Windows (PowerShell)

```powershell
& ([scriptblock]::Create((irm "https://raw.githubusercontent.com/dagnele/memento/main/install.ps1")))
```

Or download a release from [GitHub Releases](https://github.com/dagnele/memento/releases/latest).

## Quick start

Build the CLI:

```bash
cargo build
```

Initialize a workspace in the current directory:

```bash
cargo run -- init
```

Start the local server in another terminal:

```bash
cargo run -- serve
```

Index some project files:

```bash
cargo run -- add README.md src
```

Search for relevant context:

```bash
cargo run -- find "workspace config loading"
```

Store a user memory item:

```bash
cargo run -- remember --namespace user --path preferences/writing-style "Prefer concise technical explanations"
```

Inspect what is stored:

```bash
cargo run -- ls
cargo run -- show mem://user/preferences/writing-style
cargo run -- cat mem://user/preferences/writing-style
```

## Workspace layout

Running `memento init` creates a local `.memento/` workspace:

```text
.memento/
  config.toml
  index.db
  user/
  agent/
```

- `config.toml` stores workspace settings such as embedding model and server port
- `index.db` stores indexed items, metadata, and vectors
- `user/` stores Memento-owned user memory files
- `agent/` stores Memento-owned agent memory files

Tracked resources use `mem://resources/...` URIs. Stored notes use `mem://user/...` and `mem://agent/...`.

## Embeddings

`memento` uses local embedding models via `fastembed`.

- Default model: `bge-base-en-v1.5`
- Supported models:
  - `bge-small-en-v1.5` - fast lightweight indexing on local machines
  - `bge-base-en-v1.5` - balanced default for English notes and docs
  - `bge-large-en-v1.5` - highest-quality English retrieval
  - `jina-embeddings-v2-base-code` - code-heavy repositories and source search
  - `nomic-embed-text-v1.5` - longer English notes and general semantic search
  - `bge-m3` - multilingual content across mixed repositories
- List models with `cargo run -- models`
- Pick one during setup with `cargo run -- init --model <model>`

Downloaded model files are cached locally. By default, the cache lives under `~/.memento/models`, or you can override it with `MEMENTO_MODEL_CACHE_DIR`.

## Common workflows

Check workspace health:

```bash
cargo run -- doctor
```

Reindex changed files:

```bash
cargo run -- reindex src/main.rs
```

Remove a tracked resource without deleting the source file:

```bash
cargo run -- rm src/main.rs
```

Delete a stored memory item and its backing file:

```bash
cargo run -- forget mem://agent/skills/refactor-cli
```

## Development

Useful commands while working on the project:

```bash
cargo fmt --check
cargo build
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

## Status

The project is currently an MVP-oriented local CLI. It is intentionally small in scope: local-first, single-binary, and focused on fast setup and simple command-driven workflows.
