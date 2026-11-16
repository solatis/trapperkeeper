-- This file creates the same table as sqlite/0004_installation.sql, whose
-- comments explain it.
CREATE TABLE installation (
    singleton    INTEGER PRIMARY KEY CHECK (singleton = 1),
    signing_salt TEXT NOT NULL
);
