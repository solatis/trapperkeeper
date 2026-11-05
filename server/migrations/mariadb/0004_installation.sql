-- The same version and columns as the SQLite and Postgres files, in the
-- MariaDB forms 0001_tenants.sql explains.
CREATE TABLE installation (
    singleton    INTEGER PRIMARY KEY CHECK (singleton = 1),
    signing_salt VARCHAR(64) NOT NULL
) DEFAULT CHARSET = utf8mb4 COLLATE = utf8mb4_bin;
