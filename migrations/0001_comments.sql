-- edge-comments v0.3.0: D1 comments schema.
CREATE TABLE IF NOT EXISTS comments (
  id         TEXT PRIMARY KEY,
  site       TEXT NOT NULL,
  thread     TEXT NOT NULL,
  parent_id  TEXT,
  author     TEXT NOT NULL,
  body       TEXT NOT NULL,
  status     TEXT NOT NULL DEFAULT 'visible'
             CHECK (status IN ('visible', 'hidden', 'deleted')),
  client_id  TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  CHECK (length(site) <= 64),
  CHECK (length(thread) <= 400),
  CHECK (length(author) <= 80),
  CHECK (length(body) <= 10000)
);

CREATE INDEX IF NOT EXISTS idx_comments_thread
  ON comments (site, thread, status, created_at, id);
CREATE INDEX IF NOT EXISTS idx_comments_parent ON comments (parent_id);
CREATE INDEX IF NOT EXISTS idx_comments_client ON comments (client_id, created_at);
