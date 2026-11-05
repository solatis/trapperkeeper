-- Facts about this installation that every instance shares. One row, and no
-- tenant column, because they belong to no tenant.
CREATE TABLE installation (
    singleton    INTEGER PRIMARY KEY CHECK (singleton = 1),
    -- The salt the signing keys are stretched with. migrate writes it once
    -- with a random value (doc/adr/0025).
    signing_salt TEXT NOT NULL
);
