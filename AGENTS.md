# AGENTS Guide

## Rust Best Practices

- Keep changes small and focused
- Prefer clear code over clever code
- Fail with useful errors
- Avoid unnecessary abstraction
- Keep behavior covered by tests when behavior changes
- Keep the worktree clean and avoid unrelated edits

## Before Finish

```bash
cargo fmt --check && cargo build
```

## Before Commit

```bash
cargo fmt
cargo fmt --check
cargo build
cargo clippy --all-targets --all-features -- -D warnings
cargo test
git status --short
```

If full `cargo test` is too broad, run relevant tests for the touched area instead.
