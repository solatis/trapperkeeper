use crate::schema::{apps, auth_tokens};
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
pub struct AuthToken<'a> {
    pub id: &'a str,
    pub app_id: i32,
    pub name: &'a str,
}
