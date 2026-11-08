use serde::{Deserialize, Serialize};
use uuid::Uuid;
use worker::Url;

pub const SCHEMA_VERSION: u32 = 1;
pub const MAX_BODY_LEN: usize = 10_000;
pub const MAX_AUTHOR_LEN: usize = 80;
pub const MAX_SITE_LEN: usize = 64;
pub const MAX_THREAD_LEN: usize = 400;
pub const MAX_LIMIT: usize = 100;
pub const DEFAULT_LIMIT: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    Root,
    Collection {
        site: String,
        thread: String,
    },
    Item {
        site: String,
        thread: String,
        id: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    Asc,
    Desc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor {
    pub created_at: i64,
    pub id: String,
}

impl Cursor {
    pub fn encode(&self) -> String {
        format!("{}:{}", self.created_at, self.id)
    }

    pub fn decode(value: &str) -> Result<Self, ApiError> {
        let (created_at, id) = value.split_once(':').ok_or_else(|| {
            ApiError::bad_request("invalid_cursor", "Cursor must be <createdAt>:<id>")
        })?;
        let created_at = created_at.parse::<i64>().map_err(|_| {
            ApiError::bad_request("invalid_cursor", "Cursor timestamp is not a number")
        })?;
        if Uuid::parse_str(id).is_err() {
            return Err(ApiError::bad_request(
                "invalid_cursor",
                "Cursor id is not a UUID",
            ));
        }
        Ok(Self {
            created_at,
            id: id.to_string(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListQuery {
    pub limit: usize,
    pub cursor: Option<Cursor>,
    pub order: Order,
}

impl Default for ListQuery {
    fn default() -> Self {
        Self {
            limit: DEFAULT_LIMIT,
            cursor: None,
            order: Order::Asc,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateComment {
    pub author: Option<String>,
    pub body: String,
    #[serde(rename = "parentId")]
    pub parent_id: Option<String>,
    #[serde(rename = "turnstileToken")]
    pub turnstile_token: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CommentRow {
    pub id: String,
    pub site: String,
    pub thread: String,
    pub parent_id: Option<String>,
    pub author: String,
    pub body: String,
    pub status: String,
    pub client_id: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentResponse {
    pub id: String,
    pub parent_id: Option<String>,
    pub author: String,
    pub body: String,
    pub created_at: i64,
    pub updated_at: i64,
}

impl From<&CommentRow> for CommentResponse {
    fn from(row: &CommentRow) -> Self {
        Self {
            id: row.id.clone(),
            parent_id: row.parent_id.clone(),
            author: row.author.clone(),
            body: row.body.clone(),
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListResponse {
    pub schema_version: u32,
    pub site: String,
    pub thread: String,
    pub comments: Vec<CommentResponse>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ErrorEnvelope {
    pub error: ErrorBody,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ErrorBody {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiError {
    pub status: u16,
    pub code: &'static str,
    pub message: String,
}

impl ApiError {
    pub fn new(status: u16, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }

    pub fn bad_request(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(400, code, message)
    }

    pub fn forbidden(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(403, code, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(404, "not_found", message)
    }

    pub fn method_not_allowed(allow: &str) -> Self {
        Self::new(
            405,
            "method_not_allowed",
            format!("Method not allowed. Allowed: {allow}"),
        )
    }

    pub fn rate_limited(message: impl Into<String>) -> Self {
        Self::new(429, "rate_limited", message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(500, "storage_error", message)
    }

    pub fn envelope(&self) -> ErrorEnvelope {
        ErrorEnvelope {
            error: ErrorBody {
                code: self.code,
                message: self.message.clone(),
            },
        }
    }
}

impl From<worker::Error> for ApiError {
    fn from(error: worker::Error) -> Self {
        worker::console_error!("storage request failed: {error}");
        Self::internal("Storage request failed")
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(error: serde_json::Error) -> Self {
        Self::internal(format!("Serialization failed: {error}"))
    }
}

pub fn parse_route(path: &str) -> Result<Route, ApiError> {
    if path == "/" {
        return Ok(Route::Root);
    }

    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        return Ok(Route::Root);
    }

    let raw = trimmed.strip_prefix('/').unwrap_or(trimmed);
    let parts: Vec<&str> = raw.split('/').collect();

    let item = parts.len() >= 4 && parts[parts.len() - 2] == "comments";
    let collection = !item && parts.len() >= 3 && parts[parts.len() - 1] == "comments";

    if !item && !collection {
        let mentions_comments = parts.last() == Some(&"comments")
            || (parts.len() >= 2 && parts[parts.len() - 2] == "comments");
        if mentions_comments {
            return Err(ApiError::bad_request(
                "invalid_path",
                "Use /<site>/<thread...>/comments[/<id>]",
            ));
        }
        return Err(ApiError::not_found("Unknown route"));
    }

    for part in &parts {
        if !valid_segment(part) {
            return Err(ApiError::bad_request(
                "invalid_path",
                "Paths may only contain letters, digits, '-', '_', '.', and '~'",
            ));
        }
    }

    let site = parts[0].to_string();
    if site.len() > MAX_SITE_LEN {
        return Err(ApiError::bad_request(
            "invalid_path",
            "Site name is too long",
        ));
    }

    let thread_end = if item {
        parts.len() - 2
    } else {
        parts.len() - 1
    };
    let thread_parts = &parts[1..thread_end];
    if thread_parts.is_empty() {
        return Err(ApiError::bad_request(
            "invalid_path",
            "A thread path is required before /comments",
        ));
    }
    let thread = format!("/{}", thread_parts.join("/"));
    if thread.len() > MAX_THREAD_LEN {
        return Err(ApiError::bad_request(
            "invalid_path",
            "Thread path is too long",
        ));
    }

    if item {
        let id = parts[parts.len() - 1];
        let parsed = Uuid::parse_str(id).map_err(|_| {
            ApiError::bad_request("invalid_comment_id", "Comment id must be a UUID")
        })?;
        Ok(Route::Item {
            site,
            thread,
            id: parsed.to_string(),
        })
    } else {
        Ok(Route::Collection { site, thread })
    }
}

fn valid_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment != "."
        && segment != ".."
        && segment
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '~'))
}

pub fn parse_list_query(url: &Url) -> Result<ListQuery, ApiError> {
    let mut query = ListQuery::default();

    for (key, value) in url.query_pairs() {
        match key.as_ref() {
            "limit" => {
                let limit = value.parse::<usize>().map_err(|_| {
                    ApiError::bad_request("invalid_limit", "limit must be an integer")
                })?;
                if !(1..=MAX_LIMIT).contains(&limit) {
                    return Err(ApiError::bad_request(
                        "invalid_limit",
                        format!("limit must be between 1 and {MAX_LIMIT}"),
                    ));
                }
                query.limit = limit;
            }
            "cursor" => {
                query.cursor = Some(Cursor::decode(&value)?);
            }
            "order" => {
                query.order = match value.as_ref() {
                    "asc" => Order::Asc,
                    "desc" => Order::Desc,
                    _ => {
                        return Err(ApiError::bad_request(
                            "invalid_order",
                            "order must be 'asc' or 'desc'",
                        ));
                    }
                };
            }
            _ => {}
        }
    }

    Ok(query)
}

pub fn validate_body(body: &str) -> Result<String, ApiError> {
    let body = body.trim();
    if body.is_empty() {
        return Err(ApiError::bad_request(
            "invalid_body",
            "Comment body is required",
        ));
    }
    if body.chars().count() > MAX_BODY_LEN {
        return Err(ApiError::bad_request(
            "invalid_body",
            format!("Comment body must be at most {MAX_BODY_LEN} characters"),
        ));
    }
    Ok(body.to_string())
}

pub fn validate_author(author: Option<String>) -> Result<String, ApiError> {
    let author = author.unwrap_or_default();
    let author = author.trim();
    if author.is_empty() {
        return Ok("Anonymous".to_string());
    }
    if author.chars().count() > MAX_AUTHOR_LEN {
        return Err(ApiError::bad_request(
            "invalid_author",
            format!("Author must be at most {MAX_AUTHOR_LEN} characters"),
        ));
    }
    Ok(author.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_root_and_comments_routes() {
        assert_eq!(parse_route("/").unwrap(), Route::Root);
        assert_eq!(
            parse_route("/blog/my-post/comments").unwrap(),
            Route::Collection {
                site: "blog".to_string(),
                thread: "/my-post".to_string(),
            }
        );
        assert_eq!(
            parse_route("/blog/2026/my-post/comments").unwrap(),
            Route::Collection {
                site: "blog".to_string(),
                thread: "/2026/my-post".to_string(),
            }
        );
        assert_eq!(
            parse_route("/blog/my-post/comments/").unwrap(),
            Route::Collection {
                site: "blog".to_string(),
                thread: "/my-post".to_string(),
            }
        );

        let id = Uuid::now_v7();
        let route = parse_route(&format!("/blog/my-post/comments/{id}")).unwrap();
        assert_eq!(
            route,
            Route::Item {
                site: "blog".to_string(),
                thread: "/my-post".to_string(),
                id: id.to_string(),
            }
        );
    }

    #[test]
    fn rejects_bad_routes() {
        assert_eq!(parse_route("/blog/my-post").unwrap_err().status, 404);
        assert_eq!(
            parse_route("/blog//my-post/comments").unwrap_err().status,
            400
        );
        assert_eq!(
            parse_route("/blog/my-post/comments/not-a-uuid")
                .unwrap_err()
                .status,
            400
        );
        assert_eq!(parse_route("/blog/comments").unwrap_err().status, 400);
        assert_eq!(parse_route("/blog/../comments").unwrap_err().status, 400);
        assert_eq!(
            parse_route("/blog/my%20post/comments").unwrap_err().status,
            400
        );
    }

    #[test]
    fn parses_list_query() {
        let url = Url::parse("https://example.com/blog/post/comments?limit=10&order=desc").unwrap();
        let query = parse_list_query(&url).unwrap();
        assert_eq!(query.limit, 10);
        assert_eq!(query.order, Order::Desc);
        assert!(query.cursor.is_none());

        let url = Url::parse("https://example.com/x?limit=0").unwrap();
        assert!(parse_list_query(&url).is_err());
        let url = Url::parse("https://example.com/x?limit=101").unwrap();
        assert!(parse_list_query(&url).is_err());
        let url = Url::parse("https://example.com/x?order=sideways").unwrap();
        assert!(parse_list_query(&url).is_err());
    }

    #[test]
    fn cursor_round_trips() {
        let cursor = Cursor {
            created_at: 1_700_000_000_000,
            id: Uuid::now_v7().to_string(),
        };
        let encoded = cursor.encode();
        assert_eq!(Cursor::decode(&encoded).unwrap(), cursor);
        assert!(Cursor::decode("nope").is_err());
        assert!(Cursor::decode("123:not-a-uuid").is_err());
    }

    #[test]
    fn validates_body_and_author() {
        assert_eq!(validate_body("  hello  ").unwrap(), "hello");
        assert!(validate_body("   ").is_err());
        assert!(validate_body(&"x".repeat(MAX_BODY_LEN + 1)).is_err());
        assert_eq!(validate_author(None).unwrap(), "Anonymous");
        assert_eq!(validate_author(Some("  Ada  ".into())).unwrap(), "Ada");
        assert!(validate_author(Some("x".repeat(MAX_AUTHOR_LEN + 1))).is_err());
    }
}
