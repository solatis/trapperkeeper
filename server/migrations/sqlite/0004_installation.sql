-- Facts about this installation that every server on this database shares.
-- The table holds one row and no tenant column, because the facts belong to
-- no tenant.
CREATE TABLE installation (
    singleton    INTEGER PRIMARY KEY CHECK (singleton = 1),
    -- The server stretches the signing keys with this salt (ADR-0025).
    -- store::migrate::apply writes a random salt after the migrations when
    -- the installation has none, so the first run of serve or migrate writes
    -- it.
    signing_salt TEXT NOT NULL
);
