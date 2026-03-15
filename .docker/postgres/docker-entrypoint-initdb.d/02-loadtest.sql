-- Bench table with text columns for sensitive data detection testing.
-- Column types chosen to exercise pgsense scanning (TEXT/VARCHAR).
-- Non-text columns (id, ts) are skipped by the scanner automatically.

CREATE TABLE bench_sensitive (
    id         SERIAL PRIMARY KEY,
    full_name  TEXT NOT NULL,
    email      TEXT,
    phone      TEXT,
    ssn        TEXT,
    credit_card TEXT,
    notes      TEXT,
    ts         TIMESTAMPTZ DEFAULT now()
);
