use crate::schema::{apps, auth_tokens};
use crate::utils;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Queryable, Insertable)]
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

#[derive(Debug, PartialEq, Queryable, Serialize, Deserialize)]
#[diesel(table_name = apps)]
pub struct App {
    pub id: Option<i32>,
    pub name: String,
}

#[derive(Debug, PartialEq, Queryable, Insertable)]
#[diesel(table_name = auth_tokens)]
pub struct NewAuthToken {
    pub app_id: i32,
    pub name: String,
}

impl NewAuthToken {
    pub fn new(app_id: i32, name: &String) -> Self {
        NewAuthToken {
            app_id: app_id,
            name: name.to_string(),
        }
    }
}

#[derive(Debug, PartialEq, Queryable, Insertable)]
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
