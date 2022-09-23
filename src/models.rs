use crate::schema::{apps, auth_tokens};
use crate::utils;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Login {
    pub username: String,
    pub password: String,
}

impl Login {
    pub fn new(username: &String, password: &String) -> Self {
        Login {
            username: username.clone(),
            password: password.clone(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub username: String,
}

impl Session {
    pub fn new(username: &String) -> Self {
        Session {
            id: utils::random_token(32),
            username: username.clone(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Queryable, Insertable)]
#[diesel(table_name = apps)]
pub struct NewApp {
    pub name: String,
}

impl NewApp {
    pub fn new(name: &String) -> Self {
        NewApp { name: name.clone() }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Queryable)]
#[diesel(table_name = apps)]
pub struct App {
    pub id: Option<i32>,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Queryable, Insertable)]
#[diesel(table_name = auth_tokens)]
pub struct NewAuthToken {
    pub app_id: i32,
    pub name: String,
}

impl NewAuthToken {
    pub fn new(app_id: i32, name: &String) -> Self {
        NewAuthToken {
            app_id: app_id,
            name: name.clone(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Queryable, Insertable)]
#[diesel(table_name = auth_tokens)]
pub struct AuthToken {
    pub id: String,
    pub app_id: i32,
    pub name: String,
}

impl AuthToken {
    pub fn new(app_id: i32, name: &String) -> Self {
        AuthToken {
            id: utils::random_token(32),
            app_id: app_id,
            name: name.clone(),
        }
    }
}
