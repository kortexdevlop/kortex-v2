-- AI usage detail for fairness and for the owner's dashboard. Aggregates only: never prompts or answers.
CREATE TABLE ai_user_day (         -- what each account spent today (a cap stops a single heavy user)
  day      TEXT NOT NULL,
  user_id  TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  requests INTEGER NOT NULL DEFAULT 0,
  neurons  REAL NOT NULL DEFAULT 0,
  PRIMARY KEY (day, user_id)
);
CREATE INDEX ai_user_day_day ON ai_user_day(day);

CREATE TABLE ai_app (              -- which app asked (mind, flow, desktop…), from the request origin
  day      TEXT NOT NULL,
  app      TEXT NOT NULL,
  requests INTEGER NOT NULL DEFAULT 0,
  neurons  REAL NOT NULL DEFAULT 0,
  PRIMARY KEY (day, app)
);
