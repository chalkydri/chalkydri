//!
//! JSON API used by the web UI and possibly third-party applications
//!

use minint::NtConn;
use utopia::OpenApi;
use actix_web::{web::{self, Data}, App, HttpServer, Responder};

use crate::config::CameraResolution;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Chalkydri Manager API",
    ),
    paths(
        info,
        configurations,
        configure,
    ),
)]
#[allow(dead_code)]
struct ApiDoc;
 
pub async fn run_api<'nt>(nt: NtConn) {
    HttpServer::new(move || {
        App::new().app_data(Data::new(nt.clone())).service(info).service(configurations)
    })
    .bind(("0.0.0.0", 6942)).unwrap()
    .run()
    .await
    .unwrap();
}


#[derive(Serialize)]
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

#[derive(Serialize, Deserialize)]
pub struct Configuration {
    dimensions: CameraResolution,
    frame_rate: f32,
}
#[derive(Serialize)]
pub struct Configurations {
    cfgs: Vec<Configuration>,
}


/// List possible configurations
#[utopia::path(
    responses(
        (status = 200, body = Configurations),
    ),
)]
#[get("/api/configurations")]
pub(super) async fn configurations() -> impl Responder {
    web::Json(Configurations {
        cfgs: Vec::new()
    })
}

/// Set configuration
#[utopia::path(
    responses(
        (status = 200, body = web::Json),
    ),
)]
#[post("/api/configure")]
pub(super) async fn configure(web::Json(cfgg): web::Json<Configuration>) -> impl Responder {
    web::Json(serde_json::json!({
        "a": {}
    }))
}
