# Edge Comments (WIP)

This is a convention based thin wrapper around cloudflare workers kv (key-value) infrastructure written in Rust.

Store any data at path: `POST /path/to/data` with `Content-Type: text/plain` body.
Get data at path: `GET /path/to/data` as `text/plain` response.
List all keys by prefix: `GET /path/to/` (note the trailing slash) as newline (`"\n"`) delimited `text/plain` response. This contains all keys that start with `/path/to/`.

For example, if you have 3 blogs you can store comments like:
`/blog3/my-post-slug/comment-id/reply-id/second-level-reply`

I haven't added authentication yet. This should be an origin block and preferably some form of user authentication.

## References

Bootstrapped using [`workers-rs`](https://github.com/cloudflare/workers-rs).
This template is designed for compiling Rust to WebAssembly and publishing the resulting worker to
Cloudflare's [edge infrastructure](https://www.cloudflare.com/network/).

## Usage

Core logic is inside `src/lib.rs` file, acting as an entrypoint for requests hitting
the Worker.

With `wrangler`, you can build, test, and deploy your Worker with the following commands:

```bash
# compiles your project to WebAssembly and will warn of any issues
wrangler build

# run your Worker in an ideal development workflow (with a local server, file watcher & more)
wrangler dev

# deploy your Worker globally to the Cloudflare network (update your wrangler.toml file for configuration)
wrangler publish
```

Read the latest `worker` crate documentation here: https://docs.rs/worker

## WebAssembly

`workers-rs` (the Rust SDK for Cloudflare Workers used in this template) is meant to be executed as
compiled WebAssembly, and as such so **must** all the code you write and depend upon. All crates and
modules used in Rust-based Workers projects have to compile to the `wasm32-unknown-unknown` triple.

Read more about this on the [`workers-rs` project README](https://github.com/cloudflare/workers-rs).

## Issues

For issues with the `edge-comments` project itself, [open an issue here](https://github.com/umstek/edge-comments/issues).

If you have any problems with the `worker` crate, please open an issue on the upstream project
issue tracker on the [`workers-rs` repository](https://github.com/cloudflare/workers-rs).
