use sqlx;

use crate::crypto;
use crate::database;
use crate::models::AuthToken;

use super::Result;

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
