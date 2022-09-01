use rand::prelude::*;

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
pub struct AuthToken {
    pub id: String,
    pub app_id: i32,
    pub name: String,
}

/// Returns a random token of length 16
fn _random_token() -> String {
    let mut rng = rand::thread_rng();
    let xs: [u8; 16] = rng.gen();

    return hex::encode(&xs);
}

impl AuthToken {
    pub fn new(app_id: i32, name: &String) -> Self {
        AuthToken {
            id: _random_token(),
            app_id: app_id,
            name: name.to_string(),
        }
    }
}
