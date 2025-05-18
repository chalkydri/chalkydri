//!
//! JSON API used by the web UI and possibly third-party applications
//!

use actix_web::{
    App, HttpResponse, HttpServer, Responder, get,
    http::{
        StatusCode,
        header::{self, CacheDirective},
    },
    post, put,
    web::{self, Data},
};
use mime_guess::from_path;
use rust_embed::Embed;
use rustix::system::RebootCommand;
use sysinfo::System;
use utopia::{OpenApi, ToSchema};

use crate::{Cfg, cameras::CameraManager, config::Config};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Chalkydri Manager API",
        version = env!("CARGO_PKG_VERSION"),
    ),
    paths(
        info,
        configuration,
        configure,
        save_configuration,
        calibration_intrinsics,
        calibration_status,
        calibration_step,
        sys_info,
        restart,
        sys_reboot,
        sys_shutdown,
    )
)]
#[allow(dead_code)]
struct ApiDoc;

#[derive(rust_embed::Embed)]
#[folder = "../../ui/build/"]
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

/// Run the API server
pub async fn run_api(cam_man: CameraManager) {
    HttpServer::new(move || {
        App::new()
            .app_data(Data::new(cam_man.clone()))
            .service(index)
            .service(info)
            .service(configuration)
            .service(configure)
            .service(save_configuration)
            .service(calibration_intrinsics)
            .service(calibration_status)
            .service(calibration_step)
            .service(restart)
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
    pub cpu_usage: u8,
    pub mem_usage: u8,
}

/// Get Chalkydri's version and system information
#[utopia::path(
    responses(
        (status = 200, body = Info),
    ),
)]
#[get("/api/info")]
pub(super) async fn info() -> impl Responder {
    let mut system = System::new();
    system.refresh_cpu_usage();
    system.refresh_memory();

    let cpu_usage = (system.global_cpu_usage() * 100.0) as u8;
    let mem_usage = ((system.used_memory() as f64 / system.total_memory() as f64) * 100.0) as u8;

    web::Json(Info {
        version: env!("CARGO_PKG_VERSION"),
        cpu_usage,
        mem_usage,
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
        } else {
            cfgg.cameras = Some(Vec::new());
            if let Some(cameras) = &mut cfgg.cameras {
                cameras.push(cam);
            }
        }
    }
    web::Json(cfgg)
}

/// Set the configuration without saving it to the disk
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

    *Cfg.write().await = cfgg;

    for cam in cam_man.devices() {
        cam_man.update_pipeline(cam.id.clone()).await;
    }

    web::Json(Cfg.read().await.clone())
}

/// Save the configuration to disk
#[utopia::path(
    responses(
        (status = 200, body = Config),
    ),
)]
#[put("/api/configuration")]
pub(super) async fn save_configuration(web::Json(cfgg): web::Json<Config>) -> impl Responder {
    let old_config = Cfg.read().await.clone();

    if cfgg.device_name != old_config.device_name {
        rustix::system::sethostname(cfgg.device_name.clone().unwrap().as_bytes()).unwrap();
    }

    cfgg.save("chalkydri.toml").await.unwrap();

    web::Json(cfgg)
}

/// Calibrate the given camera's intrinsic parameters
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

/// Complete a calibration step for the given camera
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

/// Restart Chalkydri
#[utopia::path(
    responses(
        (status = 200),
    )
)]
#[post("/api/restart")]
pub(super) async fn restart(data: web::Data<CameraManager>) -> impl Responder {
    data.restart().await;

    HttpResponse::Ok().await.unwrap()
}

/// Restart the system
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

/// Power off the system
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

/// Get system information
#[utopia::path(
    responses(
        (status = 200),
    )
)]
#[get("/api/sys/info")]
pub(super) async fn sys_info() -> impl Responder {
    let sysinfo = rustix::system::sysinfo();
    let mut system = sysinfo::System::new();
    system.refresh_cpu_usage();

    let uptime = sysinfo.uptime as u64;
    let mem_usage =
        (((sysinfo.totalram - sysinfo.freeram) as f32 / sysinfo.totalram as f32) * 100.0) as u8;

    web::Json(SysInfo { uptime, mem_usage })
}

/// Get an MJPEG camera stream for the given camera
#[get("/stream/{cam_name}")]
pub(super) async fn stream(
    path: web::Path<String>,
    data: web::Data<CameraManager>,
) -> impl Responder {
    let cam_name = path.clone();

    println!("{cam_name}");

    if let Some(mjpeg_stream) = data.mjpeg_streams().await.get(&cam_name) {
        HttpResponse::Ok()
            .append_header(header::CacheControl(vec![CacheDirective::NoCache]))
            .append_header((header::PRAGMA, "no-cache"))
            .append_header((header::EXPIRES, 0))
            .append_header((header::CONNECTION, "close"))
            .append_header((
                header::CONTENT_TYPE,
                "multipart/x-mixed-replace; boundary=frame",
            ))
            .streaming(mjpeg_stream.clone())
    } else {
        HttpResponse::NotFound().await.unwrap()
    }
}
