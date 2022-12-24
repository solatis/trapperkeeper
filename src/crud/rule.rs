use async_trait::async_trait;
use sqlx;

use crate::database;
use crate::models::{
    NewRule, NewRuleFilterField, NewRuleFilterTrapp, Rule, RuleFilterField, RuleFilterTrapp,
    RuleType,
};

use super::Result;

async fn resolve_rule_filter_trapp(
    conn: &mut impl database::IntoConnection,
    id: i64,
    name: &str,
) -> Box<dyn Rule> {
    Box::new(RuleFilterTrapp::new(id, name, 1234))
}

async fn resolve_rule_filter_field(
    conn: &mut impl database::IntoConnection,
    id: i64,
    name: &str,
) -> Box<dyn Rule> {
    Box::new(RuleFilterField::new(id, name, "key", "value"))
}

async fn resolve_rule(
    conn: &mut impl database::IntoConnection,
    id: i64,
    name: &str,
    type_: i64,
) -> Box<dyn Rule> {
    match type_.try_into().expect("Unrecognized rule type") {
        RuleType::FilterTrapp => resolve_rule_filter_trapp(conn, id, name).await,
        RuleType::FilterField => resolve_rule_filter_field(conn, id, name).await,
    }
}

pub async fn get_rules(conn: &mut impl database::IntoConnection) -> Result<Vec<Box<dyn Rule>>> {
    let recs = sqlx::query!("SELECT id, type_, name FROM rules")
        .fetch_all(conn.into_connection())
        .await?;

    let mut ret: Vec<Box<dyn Rule>> = Vec::new();

    // TODO: can we rewrite this using .iter().map(...).collect() ? async closures are a pain, because they would
    //       cause all requests to be triggered in parallel, which we don't want.
    for rec in recs.iter() {
        ret.push(resolve_rule(conn, rec.id, &rec.name, rec.type_).await);
    }

    Ok(ret)
}

#[async_trait]
pub trait CreateRule: NewRule {
    async fn create(&self, conn: &mut sqlx::SqliteConnection, id: i64) -> Result<()>;
}

#[async_trait]
impl CreateRule for NewRuleFilterTrapp {
    async fn create(&self, conn: &mut sqlx::SqliteConnection, id: i64) -> Result<()> {
        sqlx::query!(
            r#"INSERT INTO rules_filter_trapp(rule_id, trapp_id) VALUES ( ?1, ?2 )"#,
            id,
            self.trapp_id
        )
        .execute(conn)
        .await?;
        Ok(())
    }
}

#[async_trait]
impl CreateRule for NewRuleFilterField {
    async fn create(&self, conn: &mut sqlx::SqliteConnection, id: i64) -> Result<()> {
        sqlx::query!(
            r#"INSERT INTO rules_filter_field(rule_id, field_key, field_value) VALUES ( ?1, ?2, ?3 )"#,
            id,
            self.field_key,
            self.field_value,
        )
        .execute(conn)
        .await?;
        Ok(())
    }
}

pub async fn create_rule(
    conn: &mut impl database::IntoConnection,
    rule: impl CreateRule,
) -> Result<i64> {
    let name: &str = &rule.name();
    let type_: i64 = rule.type_() as i64;

    let id = sqlx::query!(
        r#"INSERT INTO rules (name, type_) VALUES ( ?1, ?2 )"#,
        name,
        type_
    )
    .execute(conn.into_connection())
    .await?
    .last_insert_rowid();

    rule.create(conn.into_connection(), id).await?;

    Ok(id)
}
