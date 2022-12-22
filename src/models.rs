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

/// Rules
///
/// Data types for different rules.
#[derive(Copy, Clone, Debug)]
pub enum RuleType {
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

pub trait NewRule {
    fn name(&self) -> &str;
    fn type_(&self) -> RuleType;
}

pub trait Rule: NewRule {
    fn id(&self) -> i64;
}

/// RuleFilterTrapp
///
/// Filter type that allows you to filter for specific trapps.

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct NewRuleFilterTrapp {
    pub name: String,
    pub trapp_id: i64,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct RuleFilterTrapp {
    pub id: i64,
    pub name: String,
    pub trapp_id: i64,
}

impl NewRule for NewRuleFilterTrapp {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_(&self) -> RuleType {
        RuleType::FilterTrapp
    }
}

impl NewRule for RuleFilterTrapp {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_(&self) -> RuleType {
        RuleType::FilterTrapp
    }
}

impl Rule for RuleFilterTrapp {
    fn id(&self) -> i64 {
        self.id
    }
}

impl RuleFilterTrapp {
    pub fn new(id: i64, name: &str, trapp_id: i64) -> Self {
        RuleFilterTrapp {
            id: id,
            name: String::from(name),
            trapp_id: trapp_id,
        }
    }
}

/// RuleFilterField
///
/// Filter type that allows you to filter for object properties (key/values).
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct NewRuleFilterField {
    pub name: String,

    pub field_key: String,
    pub field_value: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct RuleFilterField {
    pub id: i64,
    pub name: String,

    pub field_key: String,
    pub field_value: String,
}

impl NewRule for NewRuleFilterField {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_(&self) -> RuleType {
        RuleType::FilterField
    }
}

impl NewRule for RuleFilterField {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_(&self) -> RuleType {
        RuleType::FilterField
    }
}

impl Rule for RuleFilterField {
    fn id(&self) -> i64 {
        self.id
    }
}

impl RuleFilterField {
    pub fn new(id: i64, name: &str, field_key: &str, field_value: &str) -> Self {
        RuleFilterField {
            id: id,
            name: String::from(name),
            field_key: String::from(field_key),
            field_value: String::from(field_value),
        }
    }
}
