# Contributing to edge-comments

Thanks for your interest in contributing!

## Development setup

1. Install a stable [Rust toolchain](https://rustup.rs) — this repo pins it
   (including the `wasm32-unknown-unknown` target) in `rust-toolchain.toml`,
   plus `worker-build` 0.8.7 (`cargo install worker-build --version 0.8.7 --locked`).
2. Install [Node.js](https://nodejs.org) 24+ and run `npm install` (includes the
   Cloudflare `cf` CLI and Miniflare).
3. Run `cargo check` to fetch dependencies and verify the build.

To try the worker locally (`ALLOW_INSECURE_WRITES=true` is the default in `npm run dev`):

```bash
npm run dev
```

## Before submitting a PR

All of these must pass — CI runs the same commands:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
npm run deploy:dry-run
for f in migrations/*.sql; do
  sqlite3 /tmp/edge-comments.db < "$f"
  sqlite3 /tmp/edge-comments.db < "$f"
done
```

The migration is applied twice to prove it is idempotent. If you add a
migration, add matching coverage instead of editing the existing file.

## Commit style

This project uses [Conventional Commits](https://www.conventionalcommits.org/)
(`feat:`, `fix:`, `chore:`, `docs:`, ...). Keep commits atomic — one logical
change per commit.

By participating in this project, you agree to the
[Code of Conduct](CODE_OF_CONDUCT.md).
