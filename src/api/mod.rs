//!
//! JSON API used by the web UI and possibly third-party applications
//!

use actix_web::{
    get, post,
    web::{self, Data},
    App, HttpResponse, HttpServer, Responder,
};
use mime_guess::from_path;
use minint::NtConn;
use utopia::{OpenApi, ToSchema};

use crate::{config::Config, Cfg};

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
    handle_embedded_file(path.as_str())
}

pub async fn run_api<'nt>(#[cfg(feature = "ntables")] nt: NtConn) {
    HttpServer::new(move || {
        let mut app = App::new();
        #[cfg(feature = "ntables")]
        {
            app = app.app_data(Data::new(nt.clone()));
        }
        app.service(index)
            .service(info)
            .service(configuration)
            .service(configure)
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
