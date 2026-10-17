-- Expiry sweeps must use indexes, including on dormant accounts.
CREATE INDEX sessions_expiry ON sessions(expires_at);
CREATE INDEX rate_limits_window ON rate_limits(window_start);
CREATE INDEX usage_daily_day ON usage_daily(day);
