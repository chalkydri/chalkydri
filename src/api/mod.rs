//!
//! JSON API used by the web UI and possibly third-party applications
//!

use std::{fs::File, io::Write, sync::Arc};

use actix_web::{
    App, HttpResponse, HttpServer, Responder, get,
    http::StatusCode,
    post,
    web::{self, Data},
};
use mime_guess::from_path;
use rustix::system::RebootCommand;
use tokio::sync::watch;
use utopia::{OpenApi, ToSchema};

use crate::{Cfg, cameras::CameraManager, config::Config};

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

pub async fn run_api(cam_man: CameraManager) {
    HttpServer::new(move || {
        App::new()
            .app_data(Data::new(cam_man.clone()))
            .service(index)
            .service(info)
            .service(configuration)
            .service(configure)
            .service(calibration_intrinsics)
            .service(calibration_status)
            .service(calibration_step)
            .service(dist)
            .service(sys_reboot)
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
pub(super) async fn configuration(
    data: web::Data<CameraManager>,
) -> impl Responder {
    let cam_man = data.get_ref();

    let mut cfgg = Cfg.read().await.clone();
    cfgg.cameras = cam_man.devices();
    web::Json(cfgg)
}

/// Set configuration
#[utopia::path(
    responses(
        (status = 200, body = Config),
    ),
)]
#[post("/api/configuration")]
pub(super) async fn configure(
    data: web::Data<CameraManager>,
    web::Json(cfgg): web::Json<Config>,
) -> impl Responder {
    let cam_man = data.get_ref();

    let old_config = Cfg.read().await.clone();

    if cfgg.device_name != old_config.device_name {
        rustix::system::sethostname(cfgg.device_name.clone().unwrap().as_bytes()).unwrap();
    }

    {
        *Cfg.write().await = cfgg.clone();

        let mut f = File::create("chalkydri.toml").unwrap();
        let toml_cfgg = toml::to_string_pretty(&cfgg).unwrap();
        f.write_all(toml_cfgg.as_bytes()).unwrap();
        f.flush().unwrap();
    }

    web::Json(Cfg.read().await.clone())
}

#[get("/api/calibrate/intrinsics")]
pub(super) async fn calibration_intrinsics(
    data: web::Data<CameraManager>,
) -> impl Responder {
    let cam_man = data.get_ref();
    cam_man.calibrator().await.calibrate();

    HttpResponse::new(StatusCode::OK)
}

#[derive(Serialize)]
struct CalibrationStatus {
    width: u32,
    height: u32,
    current_step: usize,
    total_steps: usize,
}

#[get("/api/calibrate/status")]
pub(super) async fn calibration_status(
    data: web::Data<CameraManager>,
) -> impl Responder {
    let cam_man = data.get_ref();

    web::Json(CalibrationStatus {
        width: 1280,
        height: 720,
        current_step: 0,
        total_steps: 200,
    })
}

#[get("/api/calibrate/step")]
pub(super) async fn calibration_step(
    data: web::Data<CameraManager>,
) -> impl Responder {
    let cam_man = data.get_ref();
    let current_step = cam_man.calib_step().await;

    web::Json(CalibrationStatus {
        width: 1280,
        height: 720,
        current_step,
        total_steps: 200,
    })
}

#[post("/api/sys/reboot")]
pub(super) async fn sys_reboot() -> impl Responder {
    rustix::system::reboot(RebootCommand::Restart).unwrap();

    web::Json(())
}
