-- AI credits: per-user token bucket + global daily budget (Workers AI free tier: 10 000 neurons/day).
CREATE TABLE ai_buckets (
  user_id    TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
  tokens     REAL NOT NULL,        -- neurons currently available
  updated_at INTEGER NOT NULL
);

CREATE TABLE ai_global (
  day  TEXT PRIMARY KEY,           -- UTC date, matches the provider's 00:00 UTC reset
  used REAL NOT NULL
);

CREATE TABLE ai_ledger (           -- aggregates only: never the prompts or answers
  day      TEXT NOT NULL,
  model    TEXT NOT NULL,
  requests INTEGER NOT NULL,
  neurons  REAL NOT NULL,
  PRIMARY KEY (day, model)
);
