use std::collections::BTreeMap;

use actix_web::{Responder, web};

use crate::config::CameraResolution;

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
