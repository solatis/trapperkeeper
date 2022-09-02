use crate::schema::{apps, auth_tokens};
use crate::utils;
use diesel::prelude::*;

#[derive(Debug, Clone, Queryable, Insertable)]
#[diesel(table_name = apps)]
pub struct NewApp {
    pub name: String,
}

impl NewApp {
    pub fn new(name: &str) -> Self {
        NewApp {
            name: name.to_string(),
        }
    }
}

#[derive(Queryable)]
#[diesel(table_name = apps)]
pub struct App {
    pub id: Option<i32>,
    pub name: String,
}

#[derive(Queryable, Insertable)]
#[diesel(table_name = auth_tokens)]
pub struct AuthToken {
    pub id: String,
    pub app_id: i32,
    pub name: String,
}

impl AuthToken {
    pub fn new(app_id: i32, name: &String) -> Self {
        AuthToken {
            id: utils::random_token(),
            app_id: app_id,
            name: name.to_string(),
        }
    }
}
