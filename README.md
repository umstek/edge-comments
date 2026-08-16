# edge-comments

[![CI](https://github.com/umstek/edge-comments/actions/workflows/ci.yml/badge.svg)](https://github.com/umstek/edge-comments/actions/workflows/ci.yml)

Convention-based comments engine for the edge, built on Cloudflare Workers KV
and written in Rust via [`workers-rs`](https://github.com/cloudflare/workers-rs).

Data is stored at the exact path it is posted to, so your URL scheme doubles as
your storage scheme — no database or schema required.

## API

| Method | Path | Description |
| ------ | ---- | ----------- |
| `POST` | `/path/to/data` | Store the `text/plain` body at the path |
| `GET` | `/path/to/data` | Fetch the stored value as `text/plain` |
| `GET` | `/path/to/` | List all keys under the prefix (newline-delimited; note the trailing slash) |

For example, if you have 3 blogs you can store comments like:
`/blog3/my-post-slug/comment-id/reply-id/second-level-reply`

## Quick start

You need a stable [Rust toolchain](https://rustup.rs) with the
`wasm32-unknown-unknown` target (pinned via `rust-toolchain.toml`) and
[wrangler](https://developers.cloudflare.com/workers/wrangler/) v4+.

1. Copy `wrangler.example.toml` to `wrangler.toml` and set your KV namespace id
2. Run a local dev server: `npx wrangler dev`
3. Deploy to the Cloudflare network: `npx wrangler deploy`

## Status and limitations

- No authentication yet. This should be an origin block and preferably some
  form of user authentication — contributions welcome (see
  [CONTRIBUTING.md](CONTRIBUTING.md)).
- Licensed under [MIT](LICENSE). By participating in this project you agree to
  the [Code of Conduct](CODE_OF_CONDUCT.md).

## References

- Bootstrapped from the [`workers-rs`](https://github.com/cloudflare/workers-rs)
  template — the Rust SDK for Cloudflare Workers. Rust-based Workers compile to
  the `wasm32-unknown-unknown` target, and so must all dependencies.
- [`worker` crate documentation](https://docs.rs/worker)

## Issues

For issues with the `edge-comments` project itself,
[open an issue here](https://github.com/umstek/edge-comments/issues).

If you have any problems with the `worker` crate, please open an issue on the
upstream issue tracker on the
[`workers-rs` repository](https://github.com/cloudflare/workers-rs).
