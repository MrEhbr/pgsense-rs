-- Test table for pgsense-rs manual testing
-- One generic table with various column types for ad-hoc INSERT testing

CREATE TABLE test (
    id SERIAL PRIMARY KEY,
    text_val TEXT,
    varchar_val VARCHAR(255),
    char_val CHAR(50),
    json_val JSONB,
    int_val INTEGER,
    bigint_val BIGINT,
    float_val DOUBLE PRECISION,
    bool_val BOOLEAN,
    ts_val TIMESTAMPTZ DEFAULT now(),
    uuid_val UUID,
    bytes_val BYTEA,
    array_val TEXT[]
);

CREATE PUBLICATION pgsense_pub FOR ALL TABLES;
