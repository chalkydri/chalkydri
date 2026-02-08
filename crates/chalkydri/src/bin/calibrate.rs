use std::time::Duration;

use chalkydri::subsystems::calibration::*;
use chalkydri::cameras::providers::{CamProvider, V4l2Provider};
use cu29::bincode::config::Config;
use cu29::config::{ComponentConfig, CuConfig, CuGraph, Node};

pub struct Configurator {
    c: CuConfig,
}
impl Configurator {
    pub fn new() -> Self {
        let c = CuConfig::new_mission_type();

        Self {
            c,
        }
    }
    pub fn configure_cam(&mut self, dev_id: &str, cam_id: u8, width: u32, height: u32) {
        {
            let _ = self.c.graphs.add_mission(dev_id).unwrap();
        }
        let g = self.c.get_graph_mut(Some(dev_id)).unwrap();

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
            cam.set_param("id", dev_id.to_owned());
            cam.set_param("width", width);
            cam.set_param("height", height);

            cam_id
        };

        // GstBuffer -> CuImage conversion
        let gst_to_cu = {
            let text_id = format!("camera_{width}_{height}");
            let gst_to_cu_id = g.get_node_id_by_name(&text_id).unwrap_or_else(|| {
                let node = Node::new(&text_id, "chalkydri::cameras::GstToCuImage");
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

            apriltags.set_param("fx", 0.0);
            apriltags.set_param("fy", 0.0);
            apriltags.set_param("cx", 0.0);
            apriltags.set_param("cy", 0.0);

            apriltags_id
        };

        // AprilTag adapter (sends data back to the bot)
        let april_adap = {
            let text_id = format!("april_adap_{dev_id}");
            let april_adap_id = g.get_node_id_by_name(&text_id).unwrap_or_else(|| {
                let node = Node::new(&text_id, "chalkydri::AprilAdapter");
                g.add_node(node).expect("this should never fail")
            });
            let april_adap = g.get_node_mut(april_adap_id).expect("very wonk config");

            april_adap.set_param("cam_id", cam_id);

            april_adap_id
        };
        
        // Make all the connections
        for (src, target, msg) in [
            (cam, gst_to_cu, "(cu_gstreamer::CuGstBuffer, CuDuration)"),
            (gst_to_cu, apriltags, "(cu_sensor_payloads::CuImage<Vec<u8>>, CuDuration)"),
            (apriltags, april_adap, "(whacknet::RobotPose, CuDuration)"),
        ] {
            if !g.connection_exists(src, target) {
                g.connect_ext(src, target, msg, Some(vec![dev_id.to_owned()]), None, None).expect("why");
            }
        }

        g.0.shrink_to_fit();
    }
    pub fn save(self) -> String {
        self.c.validate_logging_config().unwrap();
        self.c.serialize_ron()
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
    for dev_id in provider.devices() {
        println!(" > configuring {dev_id}...");
        config.configure_cam(&dev_id, 1, 1280, 720);
    }

    println!("{}", config.save());
}
