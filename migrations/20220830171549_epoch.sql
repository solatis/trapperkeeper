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

CREATE TABLE rules(
       id INTEGER PRIMARY KEY AUTOINCREMENT,
       type_ INTEGER NOT NULL,
       name TEXT NOT NULL);

CREATE TABLE rules_filter_trapp(
       rule_id INTEGER NOT NULL,
       trapp_id INTEGER NOT NULL,
       PRIMARY KEY (rule_id, trapp_id),
       FOREIGN KEY (rule_id) REFERENCES rules(id) ON UPDATE CASCADE ON DELETE CASCADE,
       FOREIGN KEY (trapp_id) REFERENCES trapps(id) ON UPDATE CASCADE ON DELETE CASCADE);

CREATE TABLE rules_filter_field(
       rule_id INTEGER NOT NULL,
       field_key TEXT NOT NULL,
       field_value TEXT NOT NULL,
       PRIMARY KEY (rule_id, field_key, field_value),
       FOREIGN KEY (rule_id) REFERENCES rules(id) ON UPDATE CASCADE ON DELETE CASCADE);
