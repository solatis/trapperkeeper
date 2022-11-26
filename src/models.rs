use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Login {
    pub username: String,
    pub password: String,
}

impl Login {
    pub fn new(username: &str, password: &str) -> Self {
        Login {
            username: String::from(username),
            password: String::from(password),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub username: String,
}

impl Session {
    pub fn new(id: &str, username: &str) -> Self {
        Session {
            id: String::from(id),
            username: String::from(username),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NewTrapp {
    pub name: String,
}

impl NewTrapp {
    pub fn new(name: &str) -> Self {
        NewTrapp {
            name: String::from(name),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Trapp {
    pub id: i64,
    pub name: String,
}

impl Trapp {
    pub fn new(id: i64, name: &str) -> Self {
        Trapp {
            id: id,
            name: String::from(name),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct NewAuthToken {
    pub name: String,
}

impl NewAuthToken {
    pub fn new(name: &str) -> Self {
        NewAuthToken {
            name: String::from(name),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AuthToken {
    pub id: String,
    pub trapp_id: i64,
    pub name: String,
}

impl AuthToken {
    pub fn new(id: &str, trapp_id: i64, name: &str) -> Self {
        AuthToken {
            id: String::from(id),
            trapp_id: trapp_id,
            name: String::from(name),
        }
    }
}
