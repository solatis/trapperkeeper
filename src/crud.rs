use async_trait::async_trait;
use derive_more;
use sqlx;
use sqlx::Database;
use std::convert::TryFrom;

use crate::crypto;
use crate::database;
use crate::models::{AuthToken, NewRule, Rule, RuleFilterField, RuleFilterTrapp, RuleType, Trapp};

#[derive(Debug, derive_more::From, derive_more::Display, derive_more::Error)]
pub enum Error {
    #[from]
    SqlxError(sqlx::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub async fn create_trapp(conn: &mut impl database::IntoConnection, title: &str) -> Result<i64> {
    let conn = conn.into_connection();

    let id = sqlx::query!(r#"INSERT INTO trapps (name) VALUES ( ?1 )"#, title)
        .execute(conn)
        .await?
        .last_insert_rowid();

    Ok(id)
}

pub async fn get_trapps(conn: &mut impl database::IntoConnection) -> Result<Vec<Trapp>> {
    let conn = conn.into_connection();

    let recs = sqlx::query!("SELECT id, name FROM trapps")
        .fetch_all(conn)
        .await?;
    let result = recs
        .iter()
        .map(|rec| Trapp::new(rec.id, &rec.name))
        .collect();
    Ok(result)
}

pub async fn get_trapp_by_id(
    conn: &mut impl database::IntoConnection,
    id: &i64,
) -> Result<Option<Trapp>> {
    let conn = conn.into_connection();

    let rec = sqlx::query!(r#"SELECT id, name FROM trapps WHERE id = ?1 "#, id)
        .fetch_optional(conn)
        .await?;

    match rec {
        Some(rec_) => Ok(Some(Trapp::new(rec_.id, &rec_.name))),
        None => Ok(None),
    }
}

pub async fn delete_trapp_by_id(
    conn: &mut impl database::IntoConnection,
    id: &i64,
) -> Result<bool> {
    let conn = conn.into_connection();

    let n = sqlx::query!(r#"DELETE FROM trapps WHERE id = ?1"#, id)
        .execute(conn)
        .await?
        .rows_affected();

    Ok(n > 0)
}

pub async fn create_auth_token(
    conn: &mut impl database::IntoConnection,
    trapp_id: &i64,
    name: &str,
) -> Result<String> {
    let conn = conn.into_connection();

    let id = crypto::random_token(32);

    sqlx::query!(
        r#"INSERT INTO auth_tokens (id, trapp_id, name) VALUES ( ?1, ?2, ?3 )"#,
        id,
        trapp_id,
        name
    )
    .execute(conn)
    .await?;

    Ok(id)
}

pub async fn get_auth_token_by_id(
    conn: &mut impl database::IntoConnection,
    id: &str,
) -> Result<Option<AuthToken>> {
    let conn = conn.into_connection();

    let rec = sqlx::query!(
        r#"SELECT id, trapp_id, name FROM auth_tokens WHERE id = ?1 "#,
        id
    )
    .fetch_optional(conn)
    .await?;

    match rec {
        Some(rec_) => Ok(Some(AuthToken::new(&rec_.id, rec_.trapp_id, &rec_.name))),
        None => Ok(None),
    }
}

pub async fn get_auth_token_by_trapp_and_id(
    conn: &mut impl database::IntoConnection,
    trapp_id: &i64,
    id: &str,
) -> Result<Option<AuthToken>> {
    let conn = conn.into_connection();

    let rec = sqlx::query!(
        r#"SELECT id, trapp_id, name FROM auth_tokens WHERE id = ?1 AND trapp_id = ?2 "#,
        id,
        trapp_id
    )
    .fetch_optional(conn)
    .await?;

    match rec {
        Some(rec_) => Ok(Some(AuthToken::new(&rec_.id, rec_.trapp_id, &rec_.name))),
        None => Ok(None),
    }
}

pub async fn get_auth_tokens_by_trapp(
    conn: &mut impl database::IntoConnection,
    trapp_id: &i64,
) -> Result<Vec<AuthToken>> {
    let conn = conn.into_connection();

    let recs = sqlx::query!(
        r#"SELECT id, name FROM auth_tokens WHERE trapp_id = ?1"#,
        trapp_id
    )
    .fetch_all(conn)
    .await?;
    let result = recs
        .iter()
        .map(|rec| AuthToken::new(&rec.id, *trapp_id, &rec.name))
        .collect();
    Ok(result)
}

pub async fn delete_auth_token_by_id(
    conn: &mut impl database::IntoConnection,
    id: &str,
) -> Result<bool> {
    let conn = conn.into_connection();

    let n = sqlx::query!(r#"DELETE FROM auth_tokens WHERE id = ?1"#, id)
        .execute(conn)
        .await?
        .rows_affected();

    Ok(n > 0)
}

pub async fn delete_auth_token_by_trapp_and_id(
    conn: &mut impl database::IntoConnection,
    trapp_id: i64,
    id: &str,
) -> Result<bool> {
    let conn = conn.into_connection();

    let n = sqlx::query!(
        r#"DELETE FROM auth_tokens WHERE id = ?1 AND trapp_id = ?2"#,
        id,
        trapp_id
    )
    .execute(conn)
    .await?
    .rows_affected();

    Ok(n > 0)
}

fn resolve_rule_filter_trapp(
    conn: &mut impl database::IntoConnection,
    id: i64,
    name: &str,
) -> Box<dyn Rule> {
    Box::new(RuleFilterTrapp::new(id, name, 1234))
}

fn resolve_rule_filter_field(
    conn: &mut impl database::IntoConnection,
    id: i64,
    name: &str,
) -> Box<dyn Rule> {
    Box::new(RuleFilterField::new(id, name, "key", "value"))
}

fn resolve_rule(
    conn: &mut impl database::IntoConnection,
    id: i64,
    name: &str,
    type_: i64,
) -> Box<dyn Rule> {
    match type_.try_into().expect("Unrecognized rule type") {
        RuleType::FilterTrapp => resolve_rule_filter_trapp(conn, id, name),
        RuleType::FilterField => resolve_rule_filter_field(conn, id, name),
    }
}

pub async fn get_rules(conn: &mut impl database::IntoConnection) -> Result<Vec<Box<dyn Rule>>> {
    let recs = sqlx::query!("SELECT id, type_, name FROM rules")
        .fetch_all(conn.into_connection())
        .await?;

    let result = recs
        .iter()
        .map(|rec| resolve_rule(conn, rec.id, &rec.name, rec.type_))
        .collect();
    Ok(result)
}

#[async_trait]
pub trait CreateRule: NewRule {
    async fn create(&self, conn: &mut sqlx::SqliteConnection, id: i64) -> Result<()>;
}

#[async_trait]
impl CreateRule for RuleFilterTrapp {
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
impl CreateRule for RuleFilterField {
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
    rule: Box<dyn CreateRule>,
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
