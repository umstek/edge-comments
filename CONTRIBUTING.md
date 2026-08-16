# Contributing to edge-comments

Thanks for your interest in contributing!

## Development setup

1. Install a stable [Rust toolchain](https://rustup.rs) — this repo pins it
   (including the `wasm32-unknown-unknown` target) in `rust-toolchain.toml`.
2. Install [Node.js](https://nodejs.org) for
   [wrangler](https://developers.cloudflare.com/workers/wrangler/) v4+.
3. Run `cargo check` to fetch dependencies and verify the build.

To try the worker locally, copy `wrangler.example.toml` to `wrangler.toml`
(gitignored) and run `npx wrangler dev`.

## Before submitting a PR

All of these must pass — CI runs the same commands:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --target wasm32-unknown-unknown --release
```

## Commit style

This project uses [Conventional Commits](https://www.conventionalcommits.org/)
(`feat:`, `fix:`, `chore:`, `docs:`, ...). Keep commits atomic — one logical
change per commit.

By participating in this project, you agree to the
[Code of Conduct](CODE_OF_CONDUCT.md).
