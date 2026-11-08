mod auth;
mod db;
mod model;

use serde::Serialize;
use uuid::Uuid;
use worker::*;

use auth::CorsPolicy;
use model::{ApiError, CommentResponse, CommentRow, CreateComment, ListResponse, Route};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ServiceInfo {
    name: &'static str,
    version: &'static str,
    schema_version: u32,
    endpoints: Vec<&'static str>,
}

fn service_info() -> ServiceInfo {
    ServiceInfo {
        name: "edge-comments",
        version: env!("CARGO_PKG_VERSION"),
        schema_version: model::SCHEMA_VERSION,
        endpoints: vec![
            "GET /",
            "GET /<site>/<thread...>/comments",
            "POST /<site>/<thread...>/comments",
            "GET /<site>/<thread...>/comments/<id>",
            "DELETE /<site>/<thread...>/comments/<id>",
        ],
    }
}

#[event(fetch)]
pub async fn main(mut req: Request, env: Env, _ctx: Context) -> Result<Response> {
    console_error_panic_hook::set_once();
    log_request(&req);

    let cors = match auth::evaluate_origin(&req, &env) {
        Ok(cors) => cors,
        Err(error) => return Ok(error_response(error, &CorsPolicy::default())),
    };

    if req.method() == Method::Options {
        return Ok(match auth::preflight_response(&cors) {
            Ok(response) => response,
            Err(error) => error_response(error, &cors),
        });
    }

    if matches!(req.method(), Method::Post | Method::Delete) && !cors.write_allowed {
        return Ok(error_response(
            ApiError::forbidden("origin_not_allowed", "Origin is not allowed"),
            &cors,
        ));
    }

    Ok(match dispatch(&mut req, &env).await {
        Ok(response) => add_cors(response, &cors),
        Err(error) => error_response(error, &cors),
    })
}

async fn dispatch(req: &mut Request, env: &Env) -> Result<Response, ApiError> {
    match model::parse_route(&req.path())? {
        Route::Root => {
            if req.method() != Method::Get {
                return Err(ApiError::method_not_allowed("GET, OPTIONS"));
            }
            json_response(200, &service_info())
        }
        Route::Collection { site, thread } => {
            if !auth::site_allowed(env, &site) {
                return Err(ApiError::forbidden(
                    "site_not_allowed",
                    "This site is not allowed",
                ));
            }
            match req.method() {
                Method::Get => handle_list(req, env, &site, &thread).await,
                Method::Post => handle_create(req, env, &site, &thread).await,
                _ => Err(ApiError::method_not_allowed("GET, POST, OPTIONS")),
            }
        }
        Route::Item { site, thread, id } => {
            if !auth::site_allowed(env, &site) {
                return Err(ApiError::forbidden(
                    "site_not_allowed",
                    "This site is not allowed",
                ));
            }
            match req.method() {
                Method::Get => handle_get(env, &site, &thread, &id).await,
                Method::Delete => handle_delete(req, env, &site, &thread, &id).await,
                _ => Err(ApiError::method_not_allowed("GET, DELETE, OPTIONS")),
            }
        }
    }
}

async fn handle_list(
    req: &Request,
    env: &Env,
    site: &str,
    thread: &str,
) -> Result<Response, ApiError> {
    let url = req.url().map_err(ApiError::from)?;
    let query = model::parse_list_query(&url)?;
    let db = env.d1("DB").map_err(ApiError::from)?;
    let page = db::list_comments(&db, site, thread, &query).await?;
    let comments: Vec<CommentResponse> = page.comments.iter().map(CommentResponse::from).collect();

    json_response(
        200,
        &ListResponse {
            schema_version: model::SCHEMA_VERSION,
            site: site.to_string(),
            thread: thread.to_string(),
            comments,
            next_cursor: page.next_cursor,
        },
    )
}

async fn handle_get(env: &Env, site: &str, thread: &str, id: &str) -> Result<Response, ApiError> {
    let db = env.d1("DB").map_err(ApiError::from)?;
    let comment = db::get_comment(&db, site, thread, id)
        .await?
        .ok_or_else(|| ApiError::not_found("Comment not found"))?;
    json_response(200, &CommentResponse::from(&comment))
}

async fn handle_create(
    req: &mut Request,
    env: &Env,
    site: &str,
    thread: &str,
) -> Result<Response, ApiError> {
    let payload: CreateComment = req
        .json()
        .await
        .map_err(|_| ApiError::bad_request("invalid_json", "Request body must be valid JSON"))?;
    let body = model::validate_body(&payload.body)?;
    let author = model::validate_author(payload.author)?;

    let remote_ip = req
        .headers()
        .get("CF-Connecting-IP")
        .map_err(ApiError::from)?;
    auth::verify_turnstile(
        env,
        payload.turnstile_token.as_deref(),
        remote_ip.as_deref(),
    )
    .await?;

    let db = env.d1("DB").map_err(ApiError::from)?;
    let parent_id = match payload.parent_id.as_deref() {
        Some(raw) => {
            let parsed = Uuid::parse_str(raw)
                .map_err(|_| ApiError::bad_request("invalid_parent", "Parent id must be a UUID"))?;
            let parent = db::get_comment(&db, site, thread, &parsed.to_string())
                .await?
                .ok_or_else(|| {
                    ApiError::bad_request("invalid_parent", "Parent comment does not exist")
                })?;
            Some(parent.id)
        }
        None => None,
    };

    let existing_client = auth::client_id(req);
    let client_id = existing_client.unwrap_or_else(Uuid::new_v4);
    let now = Date::now().as_millis() as i64;

    let recent =
        db::count_recent_by_client(&db, &client_id.to_string(), now - 10 * 60 * 1000).await?;
    if recent >= 5 {
        return Err(ApiError::rate_limited(
            "Too many comments; please try again later",
        ));
    }

    let row = CommentRow {
        id: Uuid::now_v7().to_string(),
        site: site.to_string(),
        thread: thread.to_string(),
        parent_id,
        author,
        body,
        status: "visible".to_string(),
        client_id: client_id.to_string(),
        created_at: now,
        updated_at: now,
    };
    db::insert_comment(&db, &row).await?;

    let mut response = json_response(201, &CommentResponse::from(&row))?;
    if existing_client.is_none() {
        let headers = response.headers().clone();
        headers
            .set(
                "Set-Cookie",
                &auth::client_cookie(client_id, auth::is_secure(req)),
            )
            .map_err(ApiError::from)?;
        response = response.with_headers(headers);
    }
    Ok(response)
}

async fn handle_delete(
    req: &Request,
    env: &Env,
    site: &str,
    thread: &str,
    id: &str,
) -> Result<Response, ApiError> {
    let db = env.d1("DB").map_err(ApiError::from)?;
    let comment = db::get_comment(&db, site, thread, id)
        .await?
        .ok_or_else(|| ApiError::not_found("Comment not found"))?;

    let is_admin = auth::is_admin(req, env);
    let is_owner = auth::client_id(req)
        .map(|client| client.to_string() == comment.client_id)
        .unwrap_or(false);
    if !is_admin && !is_owner {
        return Err(ApiError::forbidden(
            "not_owner",
            "You can only delete your own comment",
        ));
    }

    if !db::soft_delete(&db, site, thread, id).await? {
        return Err(ApiError::not_found("Comment not found"));
    }

    Ok(Response::empty().map_err(ApiError::from)?.with_status(204))
}

fn json_response<T: Serialize>(status: u16, value: &T) -> Result<Response, ApiError> {
    let response = Response::from_json(value)
        .map_err(ApiError::from)?
        .with_status(status);
    let headers = response.headers().clone();
    headers
        .set("Cache-Control", "no-store")
        .map_err(ApiError::from)?;
    Ok(response.with_headers(headers))
}

fn error_response(error: ApiError, cors: &CorsPolicy) -> Response {
    let response = json_response(error.status, &error.envelope()).unwrap_or_else(|_| {
        Response::error("Internal error", 500).unwrap_or_else(|_| unreachable!())
    });
    add_cors(response, cors)
}

fn add_cors(mut response: Response, cors: &CorsPolicy) -> Response {
    let headers = response.headers().clone();
    if auth::apply_cors(&headers, cors, false).is_ok() {
        response = response.with_headers(headers);
    }
    response
}

fn log_request(req: &Request) {
    worker::console_log!("{} - [{}]", Date::now().to_string(), req.path());
}
