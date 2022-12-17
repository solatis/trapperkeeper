use serde::{Deserialize, Serialize};
use std::convert::TryFrom;

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

#[derive(Copy, Clone, Debug)]
enum RuleType {
    FilterTrapp = 1,
    FilterField = 2,
}

impl TryFrom<i64> for RuleType {
    type Error = ();

    fn try_from(v: i64) -> Result<Self, Self::Error> {
        match v {
            x if x == RuleType::FilterTrapp as i64 => Ok(RuleType::FilterTrapp),
            x if x == RuleType::FilterField as i64 => Ok(RuleType::FilterField),
            _ => Err(()),
        }
    }
}

pub trait AnyRule {
    fn id(&self) -> i64;
    fn name(&self) -> &str;
    fn type_(&self) -> RuleType;
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct RuleFilterTrapp {
    pub id: i64,
    pub name: String,

    pub trapp_id: i64,
}

impl AnyRule for RuleFilterTrapp {
    fn id(&self) -> i64 {
        self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn type_(&self) -> RuleType {
        RuleType::FilterTrapp
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct RuleFilterField {
    pub id: i64,
    pub name: String,

    pub field_key: String,
    pub field_value: String,
}

impl AnyRule for RuleFilterField {
    fn id(&self) -> i64 {
        self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn type_(&self) -> RuleType {
        RuleType::FilterField
    }
}
