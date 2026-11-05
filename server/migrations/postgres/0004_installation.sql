-- The same version and columns as the SQLite and MariaDB files; the three
-- directories change together.
CREATE TABLE installation (
    singleton    INTEGER PRIMARY KEY CHECK (singleton = 1),
    signing_salt TEXT NOT NULL
);
