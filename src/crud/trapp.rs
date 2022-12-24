use sqlx;

use crate::database;
use crate::models::Trapp;

use super::Result;

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
