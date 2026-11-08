# Changelog

## 0.3.0 — 2026-11-08

### Breaking

- Workers KV is replaced by Cloudflare D1. The path→value API is replaced by a
  comments API: `/…/comments` collection and `/…/comments/<id>` item routes.
- Requests and responses are JSON; the first path segment is the site name and
  the rest is the thread path.
- Existing KV data is not migrated.

### Added

- `POST`, `GET`, and `DELETE` comment endpoints with UUIDv7 ids, soft deletes,
  replies via `parentId`, cursor pagination, and server timestamps.
- Turnstile verification for writes, anonymous `ec_client` cookie identity,
  owner-or-admin deletes, per-client rate limiting, and configurable CORS.
- D1 migration `migrations/0001_comments.sql` with thread, parent, and client
  indexes.
- Unit tests for routing, validation, cursors, cookies, CORS helpers, and
  Turnstile response parsing.

### Changed

- Dependencies: `worker` 0.8.7 (D1 feature), `serde`/`serde_json`, `uuid` 1.29,
  `url` 2. Removed the `cfg-if` template shim and `src/utils.rs`.
- Renovate preset moved from `config:base` to `config:recommended`.

### Infrastructure

- Wrangler config uses a `DB` D1 binding and a pinned `worker-build` version.
- Deploy workflow applies D1 migrations before deploying and uses
  `wrangler-action` 4.1.3.
- CI applies the migration to SQLite twice to prove idempotence.
