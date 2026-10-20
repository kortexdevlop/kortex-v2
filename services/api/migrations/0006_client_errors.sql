-- Light error monitoring: aggregated daily counters of client-side failures per app and kind.
-- No user id, no IP, no message, no URL: only "this app failed N times today".
CREATE TABLE client_errors (
  day TEXT NOT NULL,
  app TEXT NOT NULL,
  kind TEXT NOT NULL,
  n INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (day, app, kind)
);
