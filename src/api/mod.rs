//!
//! JSON API used by the web UI and possibly third-party applications
//!

use std::{fs::File, io::Write, sync::Arc};

use actix_web::{
    App, HttpResponse, HttpServer, Responder,
    body::BodyStream,
    get,
    http::{
        StatusCode,
        header::{self, CacheDirective},
    },
    post,
    web::{self, Data},
};
use mime_guess::from_path;
use rust_embed::Embed;
use rustix::system::RebootCommand;
use tokio::{io::BufStream, sync::watch};
use utopia::{OpenApi, ToSchema};

use crate::{Cfg, cameras::CameraManager, config::Config};

#[derive(OpenApi)]
#[openapi(
    info(title = "Chalkydri Manager API"),
    paths(
        info,
        configuration,
        configure,
        calibration_intrinsics,
        calibration_status,
        calibration_step,
        sys_info,
        sys_reboot,
        sys_shutdown
    )
)]
#[allow(dead_code)]
struct ApiDoc;

#[derive(rust_embed::Embed)]
#[folder = "ui/build/"]
struct Assets;

fn handle_embedded_file(path: &str) -> HttpResponse {
    match <Assets as Embed>::get(path) {
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

#[get("/api/openapi.json")]
async fn openapi_json() -> impl Responder {
    web::Json(ApiDoc::openapi())
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
            .service(sys_reboot)
            .service(sys_shutdown)
            .service(sys_info)
            .service(openapi_json)
            .service(stream)
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
pub(super) async fn configuration(data: web::Data<CameraManager>) -> impl Responder {
    let cam_man = data.get_ref();

    let mut cfgg = Cfg.read().await.clone();
    for cam in cam_man.devices() {
        if let Some(cameras) = &mut cfgg.cameras {
            if cameras.iter().filter(|c| c.id == cam.id).next().is_none() {
                cameras.push(cam);
            }
        }
    }
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

#[utopia::path(
    responses(
        (status = 200),
    ),
)]
#[get("/api/calibrate/{cam_name}/intrinsics")]
pub(super) async fn calibration_intrinsics(
    path: web::Path<String>,
    data: web::Data<CameraManager>,
) -> impl Responder {
    let cam_name = path.to_string();

    let cam_man = data.get_ref();
    let calibrated_model = cam_man
        .calibrators()
        .await
        .get_mut(&cam_name)
        .unwrap()
        .calibrate()
        .unwrap();
    {
        let json = serde_json::to_value(calibrated_model).unwrap();
        let cfgg = &mut (*Cfg.write().await);
        if let Some(cams) = &mut cfgg.cameras {
            (*cams)
                .iter_mut()
                .filter(|cam| cam.id == cam_name)
                .next()
                .unwrap()
                .calib = Some(json);
        }
        drop(cfgg);
    }

    HttpResponse::new(StatusCode::OK)
}

#[derive(Serialize, ToSchema)]
struct CalibrationStatus {
    width: u32,
    height: u32,
    current_step: usize,
    total_steps: usize,
}

#[utopia::path(
    responses(
        (status = 200, body = CalibrationStatus),
    ),
)]
#[get("/api/calibrate/status")]
pub(super) async fn calibration_status(data: web::Data<CameraManager>) -> impl Responder {
    let cam_man = data.get_ref();

    web::Json(CalibrationStatus {
        width: 1280,
        height: 720,
        current_step: 0,
        total_steps: 200,
    })
}

#[utopia::path(
    responses(
        (status = 200),
    ),
)]
#[get("/api/calibrate/{cam_name}/step")]
pub(super) async fn calibration_step(
    path: web::Path<String>,
    data: web::Data<CameraManager>,
) -> impl Responder {
    let cam_name = path.to_string();

    let cam_man = data.get_ref();
    let current_step = cam_man.calib_step(cam_name).await;

    web::Json(CalibrationStatus {
        width: 1280,
        height: 720,
        current_step,
        total_steps: 200,
    })
}

#[utopia::path(
    responses(
        (status = 200),
    )
)]
#[post("/api/sys/reboot")]
pub(super) async fn sys_reboot() -> impl Responder {
    rustix::system::reboot(RebootCommand::Restart).unwrap();

    web::Json(())
}

#[utopia::path(
    responses(
        (status = 200),
    )
)]
#[post("/api/sys/shutdown")]
pub(super) async fn sys_shutdown() -> impl Responder {
    rustix::system::reboot(RebootCommand::PowerOff).unwrap();

    web::Json(())
}

#[derive(Serialize)]
struct SysInfo {
    uptime: u64,
    mem_usage: u8,
}

#[utopia::path(
    responses(
        (status = 200),
    )
)]
#[get("/api/sys/info")]
pub(super) async fn sys_info() -> impl Responder {
    let sysinfo = rustix::system::sysinfo();

    let uptime = sysinfo.uptime as u64;
    let mem_usage =
        (((sysinfo.totalram - sysinfo.freeram) as f32 / sysinfo.totalram as f32) * 100.0) as u8;

    web::Json(SysInfo { uptime, mem_usage })
}

#[get("/stream/{cam_name}")]
pub(super) async fn stream(
    path: web::Path<String>,
    data: web::Data<CameraManager>,
) -> impl Responder {
    let cam_name = path.clone();

    println!("{cam_name}");

    HttpResponse::Ok()
        .append_header(header::CacheControl(vec![CacheDirective::NoCache]))
        .append_header((header::PRAGMA, "no-cache"))
        .append_header((header::EXPIRES, 0))
        .append_header((header::CONNECTION, "close"))
        .append_header((
            header::CONTENT_TYPE,
            "multipart/x-mixed-replace; boundary=frame",
        ))
        .streaming(data.mjpeg_streams().await.values().next().unwrap().clone())
}
