pub mod catalog;
mod constants;
mod errors;
mod fetch;
mod router;
mod routes;
mod sitemap;

use worker::*;

#[event(fetch)]
async fn main(req: Request, env: Env, ctx: Context) -> Result<Response> {
    fetch::handle(req, env, ctx).await
}
