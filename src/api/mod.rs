//!
//! JSON API used by the web UI and possibly third-party applications
//!

use actix_web::{
    web::{self, Data},
    App, HttpServer, Responder,
    get, post,
};
use minint::NtConn;
use utopia::{OpenApi, ToSchema};

use crate::{config::Config, Cfg};

#[derive(OpenApi)]
#[openapi(
    info(title = "Chalkydri Manager API"),
    paths(info, configuration, configure),
)]
#[allow(dead_code)]
struct ApiDoc;

pub async fn run_api<'nt>(nt: NtConn) {
    HttpServer::new(move || {
        App::new()
            .app_data(Data::new(nt.clone()))
            .service(info)
            .service(configuration)
            .service(configure)
    })
    .bind(("0.0.0.0", 6942))
    .unwrap()
    .run()
    .await
    .unwrap();
}

#[derive(Serialize, ToSchema)]
pub struct Info {
    pub version: &'static str,
}

/// Chalkydri version and info
#[utopia::path(
    responses(
        (status = 200, body = Info),
    ),
)]
#[get("/api/info")]
pub(super) async fn info() -> impl Responder {
    #[cfg(feature = "python")]
    let sys = "rpi";

    web::Json(Info {
        version: env!("CARGO_PKG_VERSION"),
    })
}

/// List possible configurations
#[utopia::path(
    responses(
        (status = 200, body = Config),
    ),
)]
#[get("/api/configuration")]
pub(super) async fn configuration() -> impl Responder {
    web::Json(Cfg.read().await.clone())
}

/// Set configuration
#[utopia::path(
    responses(
        (status = 200, body = Config),
    ),
)]
#[post("/api/configure")]
pub(super) async fn configure(web::Json(cfgg): web::Json<Config>) -> impl Responder {
    *Cfg.write().await = cfgg;
    web::Json(Cfg.read().await.clone())
}
