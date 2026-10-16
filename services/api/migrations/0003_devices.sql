-- Device tokens (CLI, desktop, Store) and the installs linked to the account.
CREATE TABLE devices (
  id         TEXT PRIMARY KEY,
  user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  name       TEXT NOT NULL,
  platform   TEXT,
  token_hash TEXT NOT NULL UNIQUE,   -- SHA-256 of the bearer token: a leak does not expose live tokens
  created_at INTEGER NOT NULL,
  last_seen  INTEGER NOT NULL
);
CREATE INDEX devices_user ON devices(user_id);

CREATE TABLE installs (
  user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  device_id  TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
  tool       TEXT NOT NULL,
  version    TEXT NOT NULL,
  target     TEXT NOT NULL,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY (device_id, tool)
);
CREATE INDEX installs_user ON installs(user_id);
