-- This file creates the same table as sqlite/0004_installation.sql, in the
-- MariaDB forms that 0001_tenants.sql explains.
CREATE TABLE installation (
    singleton    INTEGER PRIMARY KEY CHECK (singleton = 1),
    signing_salt VARCHAR(64) NOT NULL
) DEFAULT CHARSET = utf8mb4 COLLATE = utf8mb4_bin;
