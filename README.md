# edge-comments

[![CI](https://github.com/umstek/edge-comments/actions/workflows/ci.yml/badge.svg)](https://github.com/umstek/edge-comments/actions/workflows/ci.yml)

Convention-based comments engine for the edge, built on Cloudflare Workers
**D1** and written in Rust via [`workers-rs`](https://github.com/cloudflare/workers-rs).

One Worker and one D1 database can serve comments for every blog/app you run.
The URL path names the site and thread; the database stores the comments.

> **v0.3.0 is a breaking release.** The Workers KV key/value API was replaced by
> a relational comments API on D1. There is no KV data migration: the old
> deployment is treated as a test instance.

## Conventions

```
/<site>/<thread...>/comments            collection
/<site>/<thread...>/comments/<id>       one comment
```

- The **first segment is the site** (`blog`, `app`, ...). The rest is the
  **thread path** (`/2026/my-post`).
- Thread paths may be nested; every segment must use letters, digits, `-`, `_`,
  `.`, or `~`. No percent-encoding, `..`, `//`, or trailing slash (a trailing
  slash is ignored).
- Comment ids are UUIDv7; `createdAt`/`updatedAt` are server-set epoch
  milliseconds.
- Bodies are stored verbatim (after trimming) and returned as strings. Render
  Markdown/plain text in your frontend.
- Deletes are soft (`status = 'deleted'`); deleted comments disappear from the
  API.

## API

| Method | Path | Description |
| ------ | ---- | ----------- |
| `GET` | `/` | Service metadata |
| `GET` | `/<site>/<thread...>/comments` | List visible comments (oldest first) |
| `POST` | `/<site>/<thread...>/comments` | Create a comment |
| `GET` | `/<site>/<thread...>/comments/<id>` | Fetch one comment |
| `DELETE` | `/<site>/<thread...>/comments/<id>` | Soft-delete (owner cookie or admin token) |
| `OPTIONS` | any | CORS preflight |

List query parameters: `limit` (1–100, default 50), `order=asc|desc`
(default `asc`), and an opaque `cursor` from a previous response.

Create body:

```json
{
  "author": "Ada",
  "body": "Nice write-up!",
  "parentId": null,
  "turnstileToken": "<token from the Turnstile widget>"
}
```

`author` is optional (defaults to `Anonymous`); `parentId` must reference a
visible comment in the same site + thread; `turnstileToken` is required when
the Worker has a `TURNSTILE_SECRET`.

Comment response:

```json
{
  "id": "018f4a2c-...",
  "parentId": null,
  "author": "Ada",
  "body": "Nice write-up!",
  "createdAt": 1760000000000,
  "updatedAt": 1760000000000
}
```

List response:

```json
{
  "schemaVersion": 1,
  "site": "blog",
  "thread": "/2026/my-post",
  "comments": [],
  "nextCursor": null
}
```

Errors use `{"error": {"code": "...", "message": "..."}}` with 400/403/404/405/
429/500 statuses.

Quick example:

```bash
curl -X POST https://comments.example.com/blog/2026/my-post/comments \
  -H 'Content-Type: application/json' \
  -d '{"author":"Ada","body":"Nice write-up!"}'

curl https://comments.example.com/blog/2026/my-post/comments
```

## Write protection (no logins)

- **Turnstile.** When `TURNSTILE_SECRET` is set, every `POST` must include a
  valid `turnstileToken`; the Worker verifies it with
  `https://challenges.cloudflare.com/turnstile/v0/siteverify`. If the secret is
  not set, writes are allowed and a warning is logged — configure it in
  production.
- **Anonymous identity.** The first successful write sets an `ec_client`
  cookie: a random UUID, `HttpOnly`, `SameSite=Lax`, and `Secure` on HTTPS,
  valid for one year. It identifies the same commenter for rate limiting and
  lets them delete their own comments. It is not an authentication credential;
  anyone with the cookie value can act as that browser, so keep it `HttpOnly`.
- **Admin moderation.** Set `ADMIN_TOKEN` as a Worker secret. Sending
  `Authorization: Bearer <token>` allows deleting any comment. If unset, admin
  deletion is disabled.
- **Rate limit.** Five comments per client id per ten minutes; then `429`.
- **CORS.** Set `ALLOWED_ORIGINS` to a comma-separated list (for example
  `https://blog.umstek.com,https://app.umstek.com`). The Worker reflects a
  matching `Origin` and enables credentials. If `ALLOWED_ORIGINS` is unset,
  reads are open and writes reflect any origin (development only).
- **Site allowlist.** Optionally set `ALLOWED_SITES` (comma-separated) to reject
  unexpected first path segments in a shared database.

## Configuration

`wrangler.example.toml` contains the D1 binding and commented vars. Copy it to
`wrangler.toml` (gitignored) and fill in the database id.

Create the database once:

```bash
npx wrangler d1 create edge-comments
# copy the database_id into wrangler.toml or the CF_D1_ID GitHub secret
npx wrangler d1 migrations apply edge-comments --remote
```

Worker secrets:

```bash
npx wrangler secret put TURNSTILE_SECRET
npx wrangler secret put ADMIN_TOKEN
```

Variables (`[vars]` in `wrangler.toml`): `ALLOWED_ORIGINS`, `ALLOWED_SITES`,
and `ALLOW_INSECURE_WRITES` (local only).

## Local development

Prerequisites: stable Rust with `wasm32-unknown-unknown` (pinned by
`rust-toolchain.toml`), Node.js, and Wrangler v4+.

```bash
cp wrangler.example.toml wrangler.toml
npx wrangler d1 migrations apply edge-comments --local
npx wrangler dev --local
```

For local writes without Turnstile, create `.dev.vars` (gitignored):

```dotenv
ALLOW_INSECURE_WRITES=true
```

Then exercise the API:

```bash
curl -X POST http://127.0.0.1:8787/blog/hello/comments \
  -H 'Content-Type: application/json' \
  -d '{"author":"Ada","body":"first!"}'
curl http://127.0.0.1:8787/blog/hello/comments
```

## Data and limits

`migrations/0001_comments.sql` creates the `comments` table and indexes on
`(site, thread, status, created_at, id)`, `parent_id`, and
`(client_id, created_at)`.

- Body ≤ 10,000 characters; author ≤ 80; site ≤ 64 bytes; thread ≤ 400 bytes.
- One D1 database serves all sites; rows are isolated by `site` + `thread`.
- D1 executes queries on the primary database by default; read replication is
  not configured. This is intentional — comments should be read-after-write.
- No IP addresses or user agents are stored. The random client id is the only
  commenter identifier.

## Development

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --target wasm32-unknown-unknown --release
```

CI also applies the migration to SQLite twice to prove it is idempotent.

## License

MIT. See [LICENSE](LICENSE). By participating you agree to the
[Code of Conduct](CODE_OF_CONDUCT.md).
