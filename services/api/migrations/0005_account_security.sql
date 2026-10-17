-- Additive: existing accounts and sessions remain valid.
ALTER TABLE credentials ADD COLUMN name TEXT NOT NULL DEFAULT 'Passkey';
ALTER TABLE credentials ADD COLUMN transports TEXT NOT NULL DEFAULT '[]';
ALTER TABLE credentials ADD COLUMN last_used INTEGER;
CREATE INDEX credentials_user ON credentials(user_id);
CREATE TABLE webauthn_challenges (
  id TEXT PRIMARY KEY,
  challenge TEXT NOT NULL,
  kind TEXT NOT NULL,
  user_id TEXT REFERENCES users(id) ON DELETE CASCADE,
  session_id TEXT,
  name TEXT,
  expires_at INTEGER NOT NULL
);
CREATE INDEX webauthn_expiry ON webauthn_challenges(expires_at);
