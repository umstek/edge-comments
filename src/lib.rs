use worker::*;

mod utils;

fn log_request(req: &Request) {
    console_log!(
        "{} - [{}], located at: {:?}, within: {}",
        Date::now().to_string(),
        req.path(),
        req.cf().coordinates().unwrap_or_default(),
        req.cf().region().unwrap_or("unknown region".into())
    );
}

#[event(fetch)]
pub async fn main(mut req: Request, env: Env) -> Result<Response> {
    log_request(&req);
    utils::set_panic_hook();

    let kv = env.kv("COMMON")?;

    match req.method() {
        Method::Get => {
            if req.path().ends_with("/") {
                let list_of_keys = kv
                    .list()
                    .prefix(req.path())
                    .execute()
                    .await?
                    .keys
                    .into_iter()
                    .map(|k| format!("{}", k.name))
                    .collect::<Vec<_>>()
                    .join("\n");
                Response::ok(list_of_keys)
            } else {
                if let Some(value) = kv.get(&req.path()).await? {
                    Response::ok(value.as_string())
                } else {
                    Response::error("Not found", 404)
                }
            }
        }
        Method::Post => {
            if req.path().ends_with("/") {
                Response::error("Cannot write here", 400)
            } else {
                kv.put(&req.path(), req.text().await?)?.execute().await?;
                Response::ok("Success")
            }
        }
        _ => Response::error("Method not allowed", 405),
    }
}
