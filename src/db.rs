use worker::wasm_bindgen::JsValue;
use worker::{D1Database, Date};

use crate::model::{ApiError, CommentRow, Cursor, ListQuery, Order};

pub const COMMENT_COLUMNS: &str =
    "id, site, thread, parent_id, author, body, status, client_id, created_at, updated_at";

fn js_str(value: &str) -> JsValue {
    JsValue::from_str(value)
}

fn js_opt(value: Option<&str>) -> JsValue {
    value.map(JsValue::from_str).unwrap_or(JsValue::NULL)
}

fn js_i64(value: i64) -> JsValue {
    JsValue::from_f64(value as f64)
}

pub async fn insert_comment_with_limit(
    db: &D1Database,
    row: &CommentRow,
    since_ms: i64,
    max_comments: i64,
) -> Result<bool, ApiError> {
    let statement = db.prepare(
        "INSERT INTO comments
           (id, site, thread, parent_id, author, body, status, client_id, created_at, updated_at)
         SELECT ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10
         WHERE (
           SELECT COUNT(*) FROM comments
           WHERE client_id = ?8 AND created_at > ?11
         ) < ?12",
    );
    let args = [
        js_str(&row.id),
        js_str(&row.site),
        js_str(&row.thread),
        js_opt(row.parent_id.as_deref()),
        js_str(&row.author),
        js_str(&row.body),
        js_str(&row.status),
        js_str(&row.client_id),
        js_i64(row.created_at),
        js_i64(row.updated_at),
        js_i64(since_ms),
        js_i64(max_comments),
    ];
    let result = statement.bind(&args)?.run().await?;
    if !result.success() {
        return Err(ApiError::internal(
            result
                .error()
                .unwrap_or_else(|| "Insert failed".to_string()),
        ));
    }
    let changed = result
        .meta()
        .ok()
        .flatten()
        .and_then(|meta| meta.changes)
        .unwrap_or(0);
    Ok(changed > 0)
}

pub async fn get_comment(
    db: &D1Database,
    site: &str,
    thread: &str,
    id: &str,
) -> Result<Option<CommentRow>, ApiError> {
    let statement = db.prepare(format!(
        "SELECT {COMMENT_COLUMNS} FROM comments
         WHERE site = ?1 AND thread = ?2 AND id = ?3 AND status = 'visible'"
    ));
    let args = [js_str(site), js_str(thread), js_str(id)];
    statement
        .bind(&args)?
        .first::<CommentRow>(None)
        .await
        .map_err(ApiError::from)
}

pub async fn get_comment_moderatable(
    db: &D1Database,
    site: &str,
    thread: &str,
    id: &str,
) -> Result<Option<CommentRow>, ApiError> {
    let statement = db.prepare(format!(
        "SELECT {COMMENT_COLUMNS} FROM comments
         WHERE site = ?1 AND thread = ?2 AND id = ?3
           AND status IN ('visible', 'hidden')"
    ));
    let args = [js_str(site), js_str(thread), js_str(id)];
    statement
        .bind(&args)?
        .first::<CommentRow>(None)
        .await
        .map_err(ApiError::from)
}

pub struct CommentPage {
    pub comments: Vec<CommentRow>,
    pub next_cursor: Option<String>,
}

pub async fn list_comments(
    db: &D1Database,
    site: &str,
    thread: &str,
    query: &ListQuery,
) -> Result<CommentPage, ApiError> {
    let comparator = match query.order {
        Order::Asc => ">",
        Order::Desc => "<",
    };
    let direction = match query.order {
        Order::Asc => "ASC",
        Order::Desc => "DESC",
    };
    let statement = db.prepare(format!(
        "SELECT {COMMENT_COLUMNS} FROM comments
         WHERE site = ?1 AND thread = ?2 AND status = 'visible'
           AND (
             ?3 IS NULL
             OR created_at {comparator} ?3
             OR (created_at = ?3 AND id {comparator} ?4)
           )
         ORDER BY created_at {direction}, id {direction}
         LIMIT ?5"
    ));

    let cursor_created = query
        .cursor
        .as_ref()
        .map(|cursor| js_i64(cursor.created_at))
        .unwrap_or(JsValue::NULL);
    let cursor_id = query
        .cursor
        .as_ref()
        .map(|cursor| js_str(&cursor.id))
        .unwrap_or(JsValue::NULL);
    let args = [
        js_str(site),
        js_str(thread),
        cursor_created,
        cursor_id,
        js_i64((query.limit + 1) as i64),
    ];

    let mut comments = statement
        .bind(&args)?
        .all()
        .await?
        .results::<CommentRow>()?;
    let has_more = comments.len() > query.limit;
    if has_more {
        comments.truncate(query.limit);
    }
    let next_cursor = if has_more {
        comments.last().map(|row| {
            Cursor {
                created_at: row.created_at,
                id: row.id.clone(),
            }
            .encode()
        })
    } else {
        None
    };

    Ok(CommentPage {
        comments,
        next_cursor,
    })
}

pub async fn soft_delete(
    db: &D1Database,
    site: &str,
    thread: &str,
    id: &str,
) -> Result<bool, ApiError> {
    let statement = db.prepare(
        "UPDATE comments
         SET status = 'deleted', updated_at = ?1
         WHERE id = ?2 AND site = ?3 AND thread = ?4
           AND status IN ('visible', 'hidden')",
    );
    let args = [
        js_i64(Date::now().as_millis() as i64),
        js_str(id),
        js_str(site),
        js_str(thread),
    ];
    let result = statement.bind(&args)?.run().await?;
    if !result.success() {
        return Err(ApiError::internal(
            result
                .error()
                .unwrap_or_else(|| "Delete failed".to_string()),
        ));
    }
    let changed = result
        .meta()
        .ok()
        .flatten()
        .and_then(|meta| meta.changes)
        .unwrap_or(0);
    Ok(changed > 0)
}
