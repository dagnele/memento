# AGENTS Guide

## Rust Best Practices

- Keep changes small and focused
- Prefer clear code over clever code
- Fail with useful errors
- Avoid unnecessary abstraction
- Keep behavior covered by tests when behavior changes
- Keep the worktree clean and avoid unrelated edits

## Before Commit

```bash
cargo fmt
cargo build
cargo clippy --all-targets --all-features -- -D warnings
```
