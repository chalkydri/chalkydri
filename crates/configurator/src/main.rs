use std::collections::{BTreeMap, HashMap};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use chalkydri::cameras::GstToCuImage;
use chalkydri::cameras::pipeline::CamPipeline;
use chalkydri::cameras::providers::{CamProvider, PROVIDER};
use chalkydri_apriltags::RobotToCamOffset;
use clap::Parser;
use color_eyre::Result;
use cu29::config::{CuConfig, Node};
use cu29::prelude::*;
use cu29_helpers::basic_copper_setup;
use dialoguer::{Input, Select};
use gstreamer::State;
use gstreamer::prelude::{DeviceExt, ElementExt, PadExt};
use indexmap::IndexMap;
use indicatif::ProgressBar;

mod monitor;
mod calibration;
use calibration::*;
use monitor::Monitor;
use monitor::MonitorBundle;

#[copper_runtime(config = "../../config/calibration.ron")]
struct App {}

#[derive(Default, serde::Deserialize, serde::Serialize)]
pub struct ConfiguratorConfig {
    cameras: IndexMap<String, CamSettings>,
    mappings: HashMap<String, String>,
}

#[derive(Default, serde::Deserialize, serde::Serialize)]
pub struct CamSettings {
    cam_id: Option<u8>,
    width: Option<u32>,
    height: Option<u32>,
    calib: Option<CalibratedModel>,
    cam_offsets: Option<RobotToCamOffset>,
}
impl CamSettings {
    pub fn is_complete(&self) -> bool {
        self.width.is_some() && self.height.is_some() && self.calib.is_some()
    }
}

pub struct Configurator {
    c: ConfiguratorConfig,
    cu: CuConfig,
    /// IDs of cameras connected to the system
    discovered_cams: IndexMap<String, Option<String>>,
}
impl Configurator {
    pub fn new() -> Self {
        let c: ConfiguratorConfig = {
            if let Some(mut f) = File::open("configurator.json").ok() {
                Some(serde_json::from_reader(&mut f).unwrap())
            } else {
                None
            }
        }
        .unwrap_or_default();

        let mut cu = CuConfig::default();

        cu.logging = Some(LoggingConfig {
            enable_task_logging: false,
            ..Default::default()
        });

        if cu.resources.len() == 0 {
            cu.resources.push(ResourceBundleConfig {
                id: "comm".to_owned(),
                provider: "whacknet::CommBundle".to_owned(),
                config: None,
                missions: None,
            });
        }

        Self {
            c,
            cu,
            discovered_cams: IndexMap::new(),
        }
    }

    /// Get a camera by its device ID
    fn cam_id_by_dev_id(&self, dev_id: impl Into<String>) -> Option<String> {
        self.c.mappings.get(&dev_id.into()).cloned()
    }

    /// Get a camera's settings by its device ID
    fn cam_by_dev_id(&self, dev_id: impl Into<String>) -> Option<&CamSettings> {
        if let Some(cam_id) = self.cam_id_by_dev_id(dev_id) {
            self.c.cameras.get(&cam_id)
        } else {
            None
        }
    }

    /// Get a **mutable reference** to a camera's settings by its device ID
    fn cam_by_dev_id_mut(&mut self, dev_id: impl Into<String>) -> Option<&mut CamSettings> {
        if let Some(cam_id) = self.cam_id_by_dev_id(dev_id) {
            self.c.cameras.get_mut(&cam_id)
        } else {
            None
        }
    }

    /// Get a camera's settings by its device ID
    fn cam_by_dev_index(&self, dev_index: usize) -> Option<&CamSettings> {
        if let Some((_, Some(cam_id))) = self.discovered_cams.get_index(dev_index) {
            self.c.cameras.get(cam_id)
        } else {
            None
        }
    }

    /// Get a **mutable reference** to a camera's settings by its device ID
    fn cam_by_dev_index_mut(&mut self, dev_index: usize) -> Option<&mut CamSettings> {
        if let Some((_, Some(cam_id))) = self.discovered_cams.get_index(dev_index) {
            self.c.cameras.get_mut(cam_id)
        } else {
            None
        }
    }

    /// Find the cameras initially
    pub fn find_cameras(&mut self) {
        PROVIDER.lock().start();
        std::thread::sleep(Duration::from_secs(2));
        self.refresh_cameras();
    }

    /// Save the Copper configuration
    pub fn save_cuconfig(&mut self) {
        for (dev_id, curr_cam) in self.c.cameras.iter() {
            let width = curr_cam.width.unwrap();
            let height = curr_cam.height.unwrap();

            let g = self.cu.get_graph_mut(None).unwrap();

            // The camera itself
            let cam = {
                let text_id = format!("camera_{dev_id}");
                let cam_id = g.get_node_id_by_name(&text_id).unwrap_or_else(|| {
                    let node = Node::new(&text_id, "CamPipeline");
                    g.add_node(node).expect("this should never fail")
                });
                let cam = g.get_node_mut(cam_id).expect("very wonk config");

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
                if let Some(ref calib) = curr_cam.calib {
                    let model = calib.inner_model().clone();
                    let calib_json = serde_json::to_string(&model).unwrap();
                    apriltags.set_param("calib", calib_json);
                }

                let robot_to_cam_json = serde_json::to_string(&curr_cam.cam_offsets.unwrap()).unwrap();
                apriltags.set_param::<String>("robot_to_cam", robot_to_cam_json);

                if let Some(cam_id) = curr_cam.cam_id {
                    apriltags.set_param("cam_id", cam_id);
                }

                apriltags_id
            };

            // Make all the connections
            for (src, target, msg) in [
                (cam, gst_to_cu, "(cu_gstreamer::CuGstBuffer, CuDuration)"),
                (
                    gst_to_cu,
                    apriltags,
                    "(cu_sensor_payloads::CuImage<Vec<u8>>, CuDuration)",
                ),
            ] {
                if !g.connection_exists(src, target) {
                    g.connect_ext(src, target, msg, None, None, None)
                        .expect("why");
                }
            }

            g.0.shrink_to_fit();
        }

        let serialized_config = self.cu.serialize_ron();
        let mut f = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open("chalkydri.ron")
            .unwrap();
        f.write_all(serialized_config.unwrap().as_bytes()).unwrap();
    }

    /// Refresh the camera list
    ///
    /// [`Self::find_cameras`] MUST be called before this method.
    fn refresh_cameras(&mut self) {
        self.discovered_cams.clear();

        for device in PROVIDER.lock().devices() {
            let config_name = self.c.mappings.get(&device).cloned();
            self.discovered_cams.insert(device, config_name);
        }
    }

    /// Run the entire camera configuration procedure
    pub fn configure_cameras(mut self) -> bool {
        loop {
            if let Some(dev_index) = Select::new()
                .with_prompt("Select a camera to configure")
                .items(self.discovered_cams.iter().map(|(k, v)| {
                    if v.is_some() {
                        format!("{k} *")
                    } else {
                        k.clone()
                    }
                }))
                .item("Save configuration")
                .default(0)
                .interact()
                .ok()
            {
                if dev_index == self.discovered_cams.len() {
                    self.save();
                    return false;
                }

                let option = Select::new()
                    .with_prompt("Which configuration would you like to use?")
                    .item("Create new configuration")
                    .items(self.c.cameras.keys())
                    .default(0)
                    .interact()
                    .unwrap();

                let dev_id = self.discovered_cams.get_index(dev_index).unwrap().0.clone();

                if option == 0 {
                    let config_name: String = Input::new()
                        .with_prompt("What would you like to call this config?")
                        .interact_text()
                        .unwrap();

                    self.c.cameras.insert(config_name.clone(), Default::default());
                    self.c.mappings.insert(dev_id.clone(), config_name.clone());
                    let (config_index, _) = self.discovered_cams.insert_full(dev_id, Some(config_name));

                    self.configure_cam_id(config_index).unwrap();
                    self.configure_cam_cap(config_index);
                    self.configure_cam_offsets(config_index).unwrap();
                } else {
                    self.discovered_cams.insert(dev_id, Some(self.c.cameras.get_index((option - 1) as usize).unwrap().0.clone()));
                }
            }
        }
    }

    /// Run the camera calibration procedure
    ///
    /// This MUST be run separately from [`Self::configure_cameras`] due to some GStreamer BS.
    pub fn configure_cam_calib(&mut self, calibration_frames: u64) -> bool {
        let cam_index = Select::new()
            .with_prompt("Select a camera to calibrate")
            .items(self.discovered_cams.values().filter_map(|v| v.to_owned()))
            .default(0)
            .interact()
            .unwrap();
        let (dev_id, Some(cam_id)) = self.discovered_cams.get_index(cam_index).unwrap() else {
            panic!();
        };
        let cam = self.c.cameras.get_mut(cam_id).unwrap();
        let width = cam.width.unwrap();
        let height = cam.height.unwrap();

        // Initialize on first call
        let mut calibrator = Calibrator::new();

        let pathbuf = PathBuf::from_str("config/calibration.ron").unwrap();
        let copper_ctx = basic_copper_setup(pathbuf.as_path(), None, true, None).unwrap();

        let mut config: CuConfig = read_configuration_str(
            include_str!("../../../config/calibration.ron").to_owned(),
            None,
        )
        .unwrap();

        let g = config.get_graph_mut(None).unwrap();

        let cam_node = g
            .get_node_mut(g.get_node_id_by_name("camera").unwrap())
            .unwrap();
        cam_node.set_param("id", dev_id.to_owned());
        cam_node.set_param("width", width);
        cam_node.set_param("height", height);

        let gst_to_cu = g
            .get_node_mut(g.get_node_id_by_name("gst_to_cu").unwrap())
            .unwrap();
        gst_to_cu.set_param("width", width);
        gst_to_cu.set_param("height", height);

        let calib_node = g
            .get_node_mut(g.get_node_id_by_name("calibrator").unwrap())
            .unwrap();
        calib_node.set_param("width", width);
        calib_node.set_param("height", height);

        let mut app = AppBuilder::new()
            .with_context(&copper_ctx)
            .with_config(config)
            .build()
            .unwrap();

        app.start_all_tasks().unwrap();
        println!("   > running calibration...");

        let progress = ProgressBar::new(calibration_frames);
        // Run until done
        while progress.position() < calibration_frames {
            progress.set_position(calibrator.process() as u64);
            app.run_one_iteration().unwrap();
        }

        let model = calibrator.calibrate();

        if let Some(model) = model {
            cam.calib = Some(CalibratedModel::from_str(
                serde_json::to_string(&model).unwrap(),
            ));
        }

        true
    }

    pub fn configure_cam_id(&mut self, camera_index: usize) -> Result<()> {
        let cam_config = self.cam_by_dev_index_mut(camera_index).unwrap();

        let cam_id: String = dialoguer::Input::new()
            .with_prompt("Cam ID")
            .default(cam_config.cam_id.map(|cam_id| cam_id.to_string()).unwrap_or_default())
            .interact_text()
            .unwrap();

        cam_config.cam_id = Some(cam_id.parse()?);

        Ok(())
    }

    pub fn configure_cam_offsets(&mut self, camera_index: usize) -> Result<()> {
        let Some(ref mut cam_config) = self.cam_by_dev_index_mut(camera_index) else {
            panic!();
        };

        println!("Camera offsets");
        let x: String = dialoguer::Input::new()
            .with_prompt(" |- Translation X")
            .default(cam_config.cam_offsets.map(|o| o.x.to_string()).unwrap_or_default())
            .interact_text()
            .unwrap();
        let y: String = dialoguer::Input::new()
            .with_prompt(" |- Translation Y")
            .default(cam_config.cam_offsets.map(|o| o.y.to_string()).unwrap_or_default())
            .interact_text()
            .unwrap();
        let z: String = dialoguer::Input::new()
            .with_prompt(" |- Translation Z")
            .default(cam_config.cam_offsets.map(|o| o.z.to_string()).unwrap_or_default())
            .interact_text()
            .unwrap();
        let roll: String = dialoguer::Input::new()
            .with_prompt(" |- Roll")
            .default(cam_config.cam_offsets.map(|o| o.roll.to_string()).unwrap_or_default())
            .interact_text()
            .unwrap();
        let pitch: String = dialoguer::Input::new()
            .with_prompt(" |- Pitch")
            .default(cam_config.cam_offsets.map(|o| o.pitch.to_string()).unwrap_or_default())
            .interact_text()
            .unwrap();
        let yaw: String = dialoguer::Input::new()
            .with_prompt(" '- Yaw")
            .default(cam_config.cam_offsets.map(|o| o.yaw.to_string()).unwrap_or_default())
            .interact_text()
            .unwrap();

        let offsets = RobotToCamOffset {
            x: x.parse()?,
            y: y.parse()?,
            z: z.parse()?,
            roll: roll.parse()?,
            pitch: pitch.parse()?,
            yaw: yaw.parse()?,
        };
        cam_config.cam_offsets = Some(offsets);

        Ok(())
    }

    pub fn configure_cam_cap(&mut self, camera_index: usize) {
        let provider = PROVIDER.lock();
        let devices = provider.devices();
        let dev_id = devices.get(camera_index).unwrap();
        let dev = provider.get_by_id(dev_id.clone()).unwrap();
        let input = dev.create_element(Some("camera")).unwrap();

        input.set_state(State::Ready).unwrap();
        let pad = input.static_pad("src").unwrap();
        let query_caps = pad.query_caps(None);
        let mut caps = Vec::new();
        for structure in query_caps.iter() {
            caps.push(structure.to_owned());
        }
        let _ = input.set_state(gstreamer::State::Null);

        let cap_index = Select::new()
            .items(caps.iter().filter_map(|structure| {
                let structure_name = structure.name();

                // Determine pixel format (handle both raw video and compressed formats)
                let pixel_format = match structure_name.as_str() {
                    "image/jpeg" => "MJPEG".to_string(),
                    "video/x-h264" => "H264".to_string(),
                    "video/x-raw" => structure
                        .get::<String>("format")
                        .unwrap_or_else(|_| "RAW".to_string()),
                    // Skip audio or other non-video streams
                    _ => {
                        return None;
                    }
                };

                // Extract resolution (skip if reported as ranges rather than fixed values)
                if let Some(width) = structure.get::<i32>("width").ok() {
                    if let Some(height) = structure.get::<i32>("height").ok() {
                        return Some(format!("{width}x{height} {pixel_format}"));
                    }
                }

                None
            }))
            .default(0)
            .interact()
            .unwrap();

        let structure = caps.get(cap_index).unwrap().clone();
        let cam_config = self.cam_by_dev_id_mut(dev_id).unwrap();
        cam_config.width = Some(structure.get::<i32>("width").unwrap() as u32);
        cam_config.height = Some(structure.get::<i32>("height").unwrap() as u32);
    }

    /// Save the configuration to disk
    pub fn save(self) {
        let config = ConfiguratorConfig {
            cameras: self.c.cameras,
            mappings: HashMap::from_iter(self.discovered_cams.clone().iter().filter_map(|(k, v)| if let Some(v) = v {
                Some((k.clone(), v.clone()))
            } else {
                None
            })),
        };
        let serialized_config = serde_json::to_string(&config);
        let mut f = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open("configurator.json")
            .unwrap();
        f.write_all(serialized_config.unwrap().as_bytes()).unwrap();
    }
}

/// Chalkydri configurator
#[derive(clap::Parser)]
#[command(version, about, long_about = None)]
pub enum Command {
    Configure,
    Generate,
    Calibrate(CmdCalibrate),
}

#[derive(clap::Args)]
#[command(version, about, long_about = None)]
pub struct CmdCalibrate {
    #[arg()]
    pub calibration_frames: u64,
}

fn main() -> Result<()> {
    color_eyre::install()?;

    let cmd = Command::parse();

    println!("Initializing...");

    // Initialize GStreamer
    match gstreamer::init() {
        Ok(()) => {}
        Err(e) => {
            panic!("gstreamer failed to initialize: {e:?}");
        }
    }

    let mut config = Configurator::new();

    match cmd {
        Command::Configure => {
            println!("Finding cameras...");
            config.find_cameras();
            config.refresh_cameras();

            config.configure_cameras();
        }
        Command::Calibrate(CmdCalibrate { calibration_frames }) => {
            config.find_cameras();
            config.refresh_cameras();

            config.configure_cam_calib(calibration_frames);
            config.save_cuconfig();
            config.save();
        }
        Command::Generate => {
            config.save_cuconfig();
        }
    }

    Ok(())
}
