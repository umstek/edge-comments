use serde::Deserialize;
use uuid::Uuid;
use worker::wasm_bindgen::JsValue;
use worker::{Env, Fetch, Headers, Method, Request, RequestInit, Response};

use crate::model::ApiError;

const TURNSTILE_VERIFY_URL: &str = "https://challenges.cloudflare.com/turnstile/v0/siteverify";
const CLIENT_COOKIE: &str = "ec_client";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorsPolicy {
    pub allow_origin: Option<String>,
    pub allow_credentials: bool,
    pub write_allowed: bool,
}

impl Default for CorsPolicy {
    fn default() -> Self {
        Self {
            allow_origin: None,
            allow_credentials: false,
            write_allowed: true,
        }
    }
}

pub fn evaluate_origin(req: &Request, env: &Env) -> Result<CorsPolicy, ApiError> {
    let origin = req.headers().get("Origin").map_err(ApiError::from)?;
    let allowed = allowed_origins(env);

    Ok(match origin {
        None => CorsPolicy::default(),
        Some(origin) => {
            if allowed.is_empty() || allowed.iter().any(|entry| entry == &origin) {
                CorsPolicy {
                    allow_origin: Some(origin),
                    allow_credentials: true,
                    write_allowed: true,
                }
            } else {
                CorsPolicy {
                    allow_origin: None,
                    allow_credentials: false,
                    write_allowed: false,
                }
            }
        }
    })
}

fn allowed_origins(env: &Env) -> Vec<String> {
    let raw = env_value(env, "ALLOWED_ORIGINS").unwrap_or_default();
    parse_origin_list(&raw)
}

pub fn site_allowed(env: &Env, site: &str) -> bool {
    let raw = env_value(env, "ALLOWED_SITES").unwrap_or_default();
    let sites = parse_origin_list(&raw);
    sites.is_empty() || sites.iter().any(|entry| entry == site)
}

fn env_value(env: &Env, name: &str) -> Option<String> {
    env.secret(name)
        .ok()
        .map(|value| value.to_string())
        .or_else(|| env.var(name).ok().map(|value| value.to_string()))
}

fn parse_origin_list(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

pub fn apply_cors(headers: &Headers, cors: &CorsPolicy, preflight: bool) -> Result<(), ApiError> {
    let Some(origin) = &cors.allow_origin else {
        return Ok(());
    };

    headers
        .set("Access-Control-Allow-Origin", origin)
        .map_err(ApiError::from)?;
    headers.set("Vary", "Origin").map_err(ApiError::from)?;
    if cors.allow_credentials {
        headers
            .set("Access-Control-Allow-Credentials", "true")
            .map_err(ApiError::from)?;
    }
    if preflight {
        headers
            .set("Access-Control-Allow-Methods", "GET, POST, DELETE, OPTIONS")
            .map_err(ApiError::from)?;
        headers
            .set(
                "Access-Control-Allow-Headers",
                "Content-Type, Authorization",
            )
            .map_err(ApiError::from)?;
        headers
            .set("Access-Control-Max-Age", "86400")
            .map_err(ApiError::from)?;
    }
    Ok(())
}

pub fn preflight_response(cors: &CorsPolicy) -> Result<Response, ApiError> {
    let headers = Headers::new();
    apply_cors(&headers, cors, true)?;
    Ok(Response::empty()
        .map_err(ApiError::from)?
        .with_status(204)
        .with_headers(headers))
}

pub fn client_id(req: &Request) -> Option<Uuid> {
    let header = req.headers().get("Cookie").ok().flatten()?;
    cookie_value(&header, CLIENT_COOKIE).and_then(|value| Uuid::parse_str(value).ok())
}

fn cookie_value<'a>(header: &'a str, name: &str) -> Option<&'a str> {
    header.split(';').find_map(|part| {
        let (key, value) = part.trim().split_once('=')?;
        (key == name).then_some(value.trim())
    })
}

pub fn client_cookie(id: Uuid, secure: bool) -> String {
    let (same_site, secure_attr) = if secure {
        ("None", "; Secure")
    } else {
        ("Lax", "")
    };
    format!(
        "{CLIENT_COOKIE}={id}; Path=/; HttpOnly; SameSite={same_site}; Max-Age=31536000{secure_attr}"
    )
}

pub fn is_secure(req: &Request) -> bool {
    req.url()
        .map(|url| url.scheme() == "https")
        .unwrap_or(false)
}

pub fn is_admin(req: &Request, env: &Env) -> bool {
    let Some(expected) = env
        .secret("ADMIN_TOKEN")
        .ok()
        .map(|value| value.to_string())
    else {
        return false;
    };
    if expected.is_empty() {
        return false;
    }
    let Some(header) = req.headers().get("Authorization").ok().flatten() else {
        return false;
    };
    let Some(token) = header.strip_prefix("Bearer ") else {
        return false;
    };
    constant_time_eq(token.as_bytes(), expected.as_bytes())
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0u8;
    for (a, b) in left.iter().zip(right) {
        difference |= a ^ b;
    }
    difference == 0
}

#[derive(Debug, Deserialize)]
struct TurnstileResponse {
    success: bool,
    #[serde(rename = "error-codes", default)]
    error_codes: Vec<String>,
}

pub async fn verify_turnstile(
    env: &Env,
    token: Option<&str>,
    remote_ip: Option<&str>,
) -> Result<(), ApiError> {
    if env_value(env, "ALLOW_INSECURE_WRITES").as_deref() == Some("true") {
        worker::console_warn!("ALLOW_INSECURE_WRITES is enabled; skipping Turnstile verification");
        return Ok(());
    }

    let secret = env_value(env, "TURNSTILE_SECRET").filter(|value| !value.is_empty());

    let Some(secret) = secret else {
        worker::console_warn!(
            "TURNSTILE_SECRET is not set; comments are not protected by Turnstile"
        );
        return Ok(());
    };

    let token = token.ok_or_else(|| {
        ApiError::bad_request("turnstile_missing", "A Turnstile token is required")
    })?;

    let body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("secret", &secret)
        .append_pair("response", token)
        .append_pair("remoteip", remote_ip.unwrap_or(""))
        .finish();

    let headers = Headers::new();
    headers
        .set("Content-Type", "application/x-www-form-urlencoded")
        .map_err(ApiError::from)?;

    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(JsValue::from_str(&body)));

    let request = Request::new_with_init(TURNSTILE_VERIFY_URL, &init).map_err(ApiError::from)?;
    let mut response = Fetch::Request(request)
        .send()
        .await
        .map_err(ApiError::from)?;
    let parsed: TurnstileResponse = response.json().await.map_err(ApiError::from)?;

    if parsed.success {
        Ok(())
    } else {
        worker::console_warn!(
            "Turnstile rejected a comment: {}",
            parsed.error_codes.join(",")
        );
        Err(ApiError::forbidden(
            "turnstile_failed",
            "Turnstile verification failed",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_origin_list() {
        assert_eq!(
            parse_origin_list("https://a.test, https://b.test ,, "),
            vec!["https://a.test".to_string(), "https://b.test".to_string()]
        );
        assert!(parse_origin_list("").is_empty());
    }

    #[test]
    fn reads_cookie_value() {
        assert_eq!(
            cookie_value("a=1; ec_client=abc; b=2", "ec_client"),
            Some("abc")
        );
        assert_eq!(cookie_value("a=1", "ec_client"), None);
        assert_eq!(cookie_value("ec_client=", "ec_client"), Some(""));
    }

    #[test]
    fn builds_cookie_header() {
        let id = Uuid::new_v4();
        let header = client_cookie(id, true);
        assert!(header.contains(&format!("ec_client={id}")));
        assert!(header.contains("HttpOnly"));
        assert!(header.contains("SameSite=None"));
        assert!(header.contains("Secure"));
        let insecure = client_cookie(id, false);
        assert!(insecure.contains("SameSite=Lax"));
        assert!(!insecure.contains("Secure"));
    }

    #[test]
    fn compares_tokens_in_constant_time_shape() {
        assert!(constant_time_eq(b"secret", b"secret"));
        assert!(!constant_time_eq(b"secret", b"secreT"));
        assert!(!constant_time_eq(b"secret", b"secret-longer"));
    }

    #[test]
    fn parses_turnstile_failures() {
        let parsed: TurnstileResponse =
            serde_json::from_str(r#"{"success":false,"error-codes":["invalid-input-response"]}"#)
                .unwrap();
        assert!(!parsed.success);
        assert_eq!(parsed.error_codes, vec!["invalid-input-response"]);
    }
}
