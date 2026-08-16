use worker::*;

mod utils;

fn log_request(req: &Request) {
    console_log!(
        "{} - [{}], located at: {:?}, within: {}",
        Date::now().to_string(),
        req.path(),
        req.cf().and_then(|cf| cf.coordinates()).unwrap_or_default(),
        req.cf()
            .and_then(|cf| cf.region())
            .unwrap_or_else(|| "unknown region".into())
    );
}

#[event(fetch)]
pub async fn main(mut req: Request, env: Env, _ctx: Context) -> Result<Response> {
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
                    .map(|k| k.name)
                    .collect::<Vec<_>>()
                    .join("\n");
                Response::ok(list_of_keys)
            } else {
                if let Some(value) = kv.get(&req.path()).text().await? {
                    Response::ok(value)
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
