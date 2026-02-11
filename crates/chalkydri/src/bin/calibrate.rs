use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use chalkydri::cameras::GstToCuImage;
use chalkydri::cameras::pipeline::CamPipeline;
use chalkydri::cameras::providers::{CamProvider, CamProviderBundle, V4l2Provider};
use chalkydri::subsystems::calibration::*;
use cu29::bincode::config::Config;
use cu29::config::{ComponentConfig, CuConfig, CuGraph, Node};
use cu29::prelude::*;
use cu29_helpers::basic_copper_setup;
use gstreamer::State;
use gstreamer::prelude::{DeviceExt, ElementExt, PadExt};

#[copper_runtime(config = "../../config/calibration.ron")]
struct App {}

fn calib_camera(dev_id: &str, width: u32, height: u32) -> CalibratedModel {
    let pathbuf = PathBuf::from_str("chalkydri.copper".into()).unwrap();
    let copper_ctx = basic_copper_setup(pathbuf.as_path(), None, true, None).unwrap();

    let mut config: CuConfig = read_configuration_str(
        include_str!("../../../../config/calibration.ron").to_owned(),
        None,
    )
    .unwrap();

    let g = config.get_graph_mut(None).unwrap();

    let cam = g
        .get_node_mut(g.get_node_id_by_name("camera").unwrap())
        .unwrap();
    cam.set_param("id", dev_id.to_owned());
    cam.set_param("width", width);
    cam.set_param("height", height);

    let gst_to_cu = g
        .get_node_mut(g.get_node_id_by_name("gst_to_cu").unwrap())
        .unwrap();
    gst_to_cu.set_param("width", width);
    gst_to_cu.set_param("height", height);

    let calib = g
        .get_node_mut(g.get_node_id_by_name("calibrator").unwrap())
        .unwrap();
    calib.set_param("width", width);
    calib.set_param("height", height);

    let mut app = AppBuilder::new()
        .with_context(&copper_ctx)
        .with_config(config)
        .build()
        .unwrap();

    app.start_all_tasks().unwrap();
    println!("   > running calibration...");

    let model: CalibratedModel;
    loop {
        app.run_one_iteration().unwrap();
        let mut lock = CALIB_RESULT.lock();
        if lock.is_some() {
            model = lock.take().unwrap();
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }

    app.stop_all_tasks().unwrap();

    model
}

pub struct Configurator {
    has_run: bool,
    c: CuConfig,
}
impl Configurator {
    pub fn new() -> Self {
        let mut c = {
            let mut buf = String::new();
            if let Some(mut f) = File::open("chalkydri.ron").ok() {
                if f.read_to_string(&mut buf).is_err() {
                    None
                } else {
                    Some(read_configuration("chalkydri.ron").unwrap())
                }
            } else {
                Some(CuConfig::new_simple_type())
            }
        }
        .unwrap();
        //

        c.logging = Some(LoggingConfig {
            enable_task_logging: false,
            ..Default::default()
        });

        if c.resources.len() == 0 {
            c.resources.push(ResourceBundleConfig {
                id: "cam_provider".to_owned(),
                provider: "CamProviderBundle".to_owned(),
                config: None,
                missions: None,
            });
            c.resources.push(ResourceBundleConfig {
                id: "comm".to_owned(),
                provider: "whacknet::CommBundle".to_owned(),
                config: None,
                missions: None,
            });
        }

        Self { has_run: false, c }
    }
    pub fn configure_cam(&mut self, dev_id: &str, cam_id: u8, width: u32, height: u32) {
        //{
        //    let _ = self.c.graphs.add_mission(dev_id).unwrap();
        //}
        //let g = self.c.get_graph_mut(Some(dev_id)).unwrap();
        let g = self.c.get_graph_mut(None).unwrap();

        // The camera itself
        let cam = {
            let text_id = format!("camera_{dev_id}");
            let cam_id = g.get_node_id_by_name(&text_id).unwrap_or_else(|| {
                let node = Node::new(&text_id, "CamPipeline");
                g.add_node(node).expect("this should never fail")
            });
            let cam = g.get_node_mut(cam_id).expect("very wonk config");

            // Needs access to the camera provider
            cam.set_resources(Some([("v4l2".to_owned(), "cam_provider.v4l2".to_owned())]));

            // Configure the camera
            cam.set_param("name", "A".to_owned());
            cam.set_param("id", dev_id.to_owned());
            cam.set_param("width", width);
            cam.set_param("height", height);

            cam_id
        };

        // GstBuffer -> CuImage conversion
        let gst_to_cu = {
            let text_id = format!("gst_to_cu_{dev_id}");
            let gst_to_cu_id = g.get_node_id_by_name(&text_id).unwrap_or_else(|| {
                let node = Node::new(&text_id, "GstToCuImage");
                g.add_node(node).expect("this should never fail")
            });
            let gst_to_cu = g.get_node_mut(gst_to_cu_id).expect("very wonk config");

            gst_to_cu.set_param("width", width);
            gst_to_cu.set_param("height", height);
            gst_to_cu.set_param("fourcc", "GREY".to_owned());

            gst_to_cu_id
        };

        // AprilTag processing
        let apriltags = {
            let text_id = format!("apriltags_{dev_id}");
            let apriltags_id = g.get_node_id_by_name(&text_id).unwrap_or_else(|| {
                let node = Node::new(&text_id, "chalkydri_apriltags::AprilTags");
                g.add_node(node).expect("this should never fail")
            });
            let apriltags = g.get_node_mut(apriltags_id).expect("very wonk config");

            apriltags.set_resources(Some([("comm".to_owned(), "comm.comm".to_owned())]));

            // Due to GStreamer being GStreamer, can only do one camera calibration per run
            if apriltags.get_param::<String>("calib").is_none() && !self.has_run {
                let model = calib_camera(dev_id, width, height);
                let calib = serde_json::to_string(&model.inner_model()).unwrap();
                apriltags.set_param("calib", calib);
                self.has_run = true;
            }

            apriltags_id
        };

        // AprilTag adapter (sends data back to the bot)
        let april_adap = {
            let text_id = format!("april_adap_{dev_id}");
            let april_adap_id = g.get_node_id_by_name(&text_id).unwrap_or_else(|| {
                let node = Node::new(&text_id, "AprilAdapter");
                g.add_node(node).expect("this should never fail")
            });
            let april_adap = g.get_node_mut(april_adap_id).expect("very wonk config");

            april_adap.set_resources(Some([("comm".to_owned(), "comm.comm".to_owned())]));

            april_adap.set_param("cam_id", cam_id);

            april_adap_id
        };

        // Make all the connections
        for (src, target, msg) in [
            (cam, gst_to_cu, "(cu_gstreamer::CuGstBuffer, CuDuration)"),
            (
                gst_to_cu,
                apriltags,
                "(cu_sensor_payloads::CuImage<Vec<u8>>, CuDuration)",
            ),
            (apriltags, april_adap, "(whacknet::RobotPose, CuDuration)"),
        ] {
            if !g.connection_exists(src, target) {
                g.connect_ext(src, target, msg, None, None, None)
                    .expect("why");
            }
        }

        g.0.shrink_to_fit();
    }
    /// Save the configuration to disk
    pub fn save(self) {
        self.c.validate_logging_config().unwrap();
        let serialized_config = self.c.serialize_ron();
        let mut f = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open("chalkydri.ron")
            .unwrap();
        f.write_all(serialized_config.as_bytes()).unwrap();
    }
}

fn main() {
    let mut config = Configurator::new();

    // Initialize GStreamer
    match gstreamer::init() {
        Ok(()) => {
            tracing::debug!("initialized gstreamer");
        }
        Err(e) => {
            panic!("gstreamer failed to initialize: {e:?}");
        }
    }

    let provider = V4l2Provider::init();
    provider.start();
    println!("finding cameras...");
    std::thread::sleep(Duration::from_secs(2));
    let mut devs = HashMap::new();
    for dev_id in provider.devices() {
        let dev = provider.get_by_id(dev_id.clone()).unwrap();
        let input = dev.create_element(Some("camera")).unwrap();

        input.set_state(State::Ready).unwrap();
        let pad = input.static_pad("src").unwrap();
        let caps = pad.query_caps(None);

        let mut best_width = 0i32;
        let mut best_height = 0i32;

        for structure in caps.iter() {
            let structure_name = structure.name();

            // Determine pixel format (handle both raw video and compressed formats)
            let pixel_format = match structure_name.as_str() {
                "image/jpeg" => "MJPEG".to_string(),
                "video/x-h264" => "H264".to_string(),
                "video/x-raw" => structure
                    .get::<String>("format")
                    .unwrap_or_else(|_| "RAW".to_string()),
                _ => continue, // Skip audio or other non-video streams
            };

            // Extract resolution (skip if reported as ranges rather than fixed values)
            let width: i32 = structure.get("width").ok().unwrap_or(0);
            let height: i32 = structure.get("height").ok().unwrap_or(0);
            if width == 0 || height == 0 {
                continue; // Skip range-based entries for simplicity
            }

            if width > best_width {
                best_width = width;
                best_height = height;

                println!("found better caps: {structure:?}");
            }
        }

        // Clean up: return to NULL state
        let _ = input.set_state(gstreamer::State::Null);

        dbg!(best_width, best_height);
        devs.insert(dev_id, (best_width, best_height));
    }
    provider.stop();
    drop(provider);

    for (cam_id, (dev_id, (width, height))) in devs.iter().enumerate() {
        println!(" > configuring {dev_id} ({width}x{height})...");
        config.configure_cam(&dev_id, cam_id as u8, (*width) as u32, (*height) as u32);
    }

    config.save();
}
