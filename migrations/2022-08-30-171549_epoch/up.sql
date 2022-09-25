-- Your SQL goes here

CREATE TABLE trapps (
       id INTEGER PRIMARY KEY AUTOINCREMENT,
       name TEXT NOT NULL
);

CREATE TABLE auth_tokens(
       id CHAR(64) PRIMARY KEY NOT NULL,
       trapp_id INTEGER NOT NULL,
       name TEXT NOT NULL,
       FOREIGN KEY (trapp_id) REFERENCES trapps(id) ON UPDATE CASCADE ON DELETE CASCADE
);
