//!
//! JSON API used by the web UI and possibly third-party applications
//!

use std::sync::Arc;

use actix_web::{
    get,
    http::StatusCode,
    post,
    web::{self, Data},
    App, HttpResponse, HttpServer, Responder,
};
use mime_guess::from_path;
use tokio::sync::watch;
use utopia::{OpenApi, ToSchema};

use crate::{
    cameras::CameraManager,
    config::Config,
    Cfg,
};

#[derive(OpenApi)]
#[openapi(
    info(title = "Chalkydri Manager API"),
    paths(info, configuration, configure)
)]
#[allow(dead_code)]
struct ApiDoc;

#[derive(rust_embed::Embed)]
#[folder = "ui/build/"]
struct Assets;

fn handle_embedded_file(path: &str) -> HttpResponse {
    match Assets::get(path) {
        Some(content) => HttpResponse::Ok()
            .content_type(from_path(path).first_or_octet_stream().as_ref())
            .body(content.data.into_owned()),
        None => HttpResponse::NotFound().body("404 Not Found"),
    }
}

#[get("/")]
async fn index() -> impl Responder {
    handle_embedded_file("index.html")
}

#[get("/{_:.*}")]
async fn dist(path: web::Path<String>) -> impl Responder {
    if Assets::get(path.as_str()).is_some() {
        handle_embedded_file(path.as_str()).map_into_boxed_body()
    } else {
        HttpResponse::TemporaryRedirect()
            .insert_header(("Location", "/"))
            .body(())
            .map_into_boxed_body()
    }
}

pub async fn run_api(cam_man: CameraManager, rx: watch::Receiver<Arc<Vec<u8>>>) {
    HttpServer::new(move || {
        App::new()
            .app_data(Data::new((cam_man.clone(), rx.clone())))
            .service(index)
            .service(info)
            .service(configuration)
            .service(configure)
            .service(calibrate)
            .service(calibration_step)
            .service(dist)
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
    let mut cfgg = Cfg.read().await.clone();
    cfgg.cameras = CameraManager::new().devices();
    web::Json(cfgg)
}

/// Set configuration
#[utopia::path(
    responses(
        (status = 200, body = Config),
    ),
)]
#[post("/api/configuration")]
pub(super) async fn configure(data: web::Data<(CameraManager, watch::Receiver<Arc<Vec<u8>>>)>, web::Json(cfgg): web::Json<Config>) -> impl Responder {
    let (cam_man, rx) = data.get_ref();
    cam_man.start();

    {
        *Cfg.write().await = cfgg;
    }

    web::Json(Cfg.read().await.clone())
}

#[get("/api/calibrate")]
pub(super) async fn calibrate(
    data: web::Data<(CameraManager, watch::Receiver<Arc<Vec<u8>>>)>,
) -> impl Responder {
    let (cam_man, rx) = data.get_ref();
    cam_man.calibrator().await.calibrate();

    HttpResponse::new(StatusCode::OK)
}
    

#[get("/api/calibrate/{width}/{height}")]
pub(super) async fn calibration_step(
    path: web::Path<(u32, u32)>,
    data: web::Data<(CameraManager, watch::Receiver<Arc<Vec<u8>>>)>,
) -> impl Responder {
    let (width, height) = *path;
    let (cam_man, rx) = data.get_ref();
    cam_man.calibrator().await.collect_data(rx.clone());

    HttpResponse::new(StatusCode::OK)
}
