extern crate cfg_if;
extern crate js_sys;
extern crate url;
extern crate wasm_bindgen;
extern crate web_sys;

mod utils;

use cfg_if::cfg_if;
use url::Url;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Headers, Request, Response, ResponseInit};

cfg_if! {
    // When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
    // allocator.
    if #[cfg(feature = "wee_alloc")] {
        extern crate wee_alloc;
        #[global_allocator]
        static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;
    }
}

#[wasm_bindgen]
extern "C" {
    type KV;

    #[wasm_bindgen(static_method_of = KV)]
    pub async fn list() -> JsValue;

    #[wasm_bindgen(static_method_of = KV)]
    pub async fn get(key: String, data_type: &str) -> JsValue;

    #[wasm_bindgen(static_method_of = KV)]
    pub async fn put(key: String, value: String);

    #[wasm_bindgen(static_method_of = KV)]
    pub async fn delete(key: String) -> JsValue;
}

fn generate_response(body: Option<String>, headers: &Headers) -> Response {
    let mut init = ResponseInit::new();
    init.headers(&JsValue::from(headers));
    if let Some(text) = body {
        init.status(200);
        Response::new_with_opt_str_and_init(Some(&text), &init).unwrap()
    } else {
        init.status(404);
        Response::new_with_opt_str_and_init(Some("Not Found"), &init).unwrap()
    }
}

async fn handle_get(key: String) -> Option<String> {
    KV::get(key, "text").await.as_string()
}

async fn handle_post(key: String, value: String) {
    KV::put(key, value).await
}

async fn get_request_body_string(request: Request) -> Option<String> {
    if let Ok(promise) = request.text() {
        if let Ok(text) = JsFuture::from(promise).await {
            text.as_string()
        } else {
            None
        }
    } else {
        None
    }
}

#[wasm_bindgen]
pub async fn main(request: Request) -> Response {
    let url = Url::parse(&request.url()).unwrap();
    // if let Origin::Tuple(scheme, Host::Domain(domain), port) = url.origin() {
    //     // CORS
    // } else {
    //     // CORS Fail?
    // }
    let key = url.path().trim_start_matches('/').to_string();
    match request.method().as_str() {
        "GET" if !key.is_empty() => {
            generate_response(handle_get(key).await, &(Headers::new().unwrap()))
        }
        "POST" if !key.is_empty() => {
            if let Some(text) = get_request_body_string(request).await {
                handle_post(key, text.clone()).await;
                generate_response(Some(text), &(Headers::new().unwrap()))
            } else {
                generate_response(None, &(Headers::new().unwrap()))
            }
        }
        _ => generate_response(None, &(Headers::new().unwrap())),
    }
}
