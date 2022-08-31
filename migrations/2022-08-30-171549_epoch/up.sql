-- Your SQL goes here

CREATE TABLE apps (
       id INTEGER PRIMARY KEY AUTOINCREMENT,
       name TEXT NOT NULL
);

CREATE TABLE auth_tokens(
       id CHAR(64) PRIMARY KEY NOT NULL,
       app_id INTEGER NOT NULL,
       name TEXT NOT NULL,
       FOREIGN KEY (app_id) REFERENCES apps(id) ON UPDATE CASCADE ON DELETE CASCADE
);
