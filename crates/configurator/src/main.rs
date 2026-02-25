use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use chalkydri::cameras::GstToCuImage;
use chalkydri::cameras::pipeline::CamPipeline;
use chalkydri::cameras::providers::{CamProvider, PROVIDER, V4l2Provider};
use chalkydri_apriltags::RobotToCamOffset;
use clap::Parser;
use cu29::config::{CuConfig, Node};
use cu29::prelude::*;
use cu29_helpers::basic_copper_setup;
use dialoguer::Select;
use gstreamer::prelude::{DeviceExt, ElementExt, PadExt};
use gstreamer::{State, Structure};

mod calibration;
use calibration::*;

#[copper_runtime(config = "../../config/calibration.ron")]
struct App {}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct ConfiguratorConfig {
    cameras: HashMap<String, CamSettings>,
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
    has_run: bool,
    c: CuConfig,
    cameras: Vec<String>,
    camera_configs: HashMap<String, CamSettings>,
    current_cam: Option<String>,
}
impl Configurator {
    pub fn new() -> Self {
        let config: ConfiguratorConfig = {
            let mut buf = String::new();
            if let Some(mut f) = File::open("configurator.json").ok() {
                Some(serde_json::from_reader(&mut f).unwrap())
            } else {
                None
            }
        }
        .unwrap();

        let mut c = CuConfig::default();

        c.logging = Some(LoggingConfig {
            enable_task_logging: false,
            ..Default::default()
        });

        if c.resources.len() == 0 {
            c.resources.push(ResourceBundleConfig {
                id: "comm".to_owned(),
                provider: "whacknet::CommBundle".to_owned(),
                config: None,
                missions: None,
            });
        }
        let cameras = config.cameras.keys().cloned().collect();

        Self {
            has_run: false,
            c,
            camera_configs: config.cameras,
            cameras,
            current_cam: None,
        }
    }

    pub fn find_cameras(&mut self) {
        PROVIDER.lock().start();
        std::thread::sleep(Duration::from_secs(2));
    }

    pub fn save_cuconfig(&mut self) {
        for (dev_id, curr_cam) in self.camera_configs.iter() {
            let width = curr_cam.width.unwrap();
            let height = curr_cam.height.unwrap();

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
                if apriltags.get_param::<String>("calib").unwrap().is_none() && !self.has_run {
                    if let Some(ref calib) = curr_cam.calib {
                        let model = calib.inner_model().clone();
                        let calib_json = serde_json::to_string(&model).unwrap();
                        apriltags.set_param("calib", calib_json);
                    }
                    self.has_run = true;
                }

                apriltags.set_param("cam_id", curr_cam.cam_id.unwrap());

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
    }

    pub fn refresh_cameras(&mut self) {
        self.camera_configs.clear();
        self.cameras.clear();

        for device in PROVIDER.lock().devices() {
            self.camera_configs
                .insert(device.clone(), Default::default());
            self.cameras.push(device);
        }
    }

    pub fn configure_cameras(mut self) -> bool {
        loop {
            if let Some(cam_id) = Select::new()
                .with_prompt("Select a camera to configure")
                .items(&self.cameras)
                .item("Save configuration")
                .default(0)
                .interact()
                .ok()
            {
                if cam_id == self.cameras.len() {
                    self.save();
                    return false;
                }

                self.configure_cam_id(cam_id).unwrap();
                self.configure_cam_cap(cam_id);
                self.configure_cam_offsets(cam_id).unwrap();
            }
        }
    }

    //pub fn build_cam_calib_view(&mut self, frame: &mut Frame, area: Rect) {
    //    let mut calibrator = Calibrator::new();
    //    let dev_id = self.current_cam.clone().unwrap();
    //    let cam = self.camera_configs.get_mut(&dev_id).unwrap();

    //    let width = cam.width.unwrap();
    //    let height = cam.height.unwrap();

    //    let pathbuf = PathBuf::from_str("chalkydri.copper".into()).unwrap();
    //    let copper_ctx = basic_copper_setup(pathbuf.as_path(), None, true, None).unwrap();

    //    let mut config: CuConfig = read_configuration_str(
    //        include_str!("../../../config/calibration.ron").to_owned(),
    //        None,
    //    )
    //    .unwrap();

    //    let g = config.get_graph_mut(None).unwrap();

    //    let cam = g
    //        .get_node_mut(g.get_node_id_by_name("camera").unwrap())
    //        .unwrap();
    //    cam.set_param("id", dev_id.to_owned());
    //    cam.set_param("width", width);
    //    cam.set_param("height", height);

    //    let gst_to_cu = g
    //        .get_node_mut(g.get_node_id_by_name("gst_to_cu").unwrap())
    //        .unwrap();
    //    gst_to_cu.set_param("width", width);
    //    gst_to_cu.set_param("height", height);

    //    let calib = g
    //        .get_node_mut(g.get_node_id_by_name("calibrator").unwrap())
    //        .unwrap();
    //    calib.set_param("width", width);
    //    calib.set_param("height", height);
    //    let mut app = AppBuilder::new()
    //        .with_context(&copper_ctx)
    //        .with_config(config)
    //        .build()
    //        .unwrap();

    //    app.start_all_tasks().unwrap();
    //    println!("   > running calibration...");

    //    let mut frames = 0usize;
    //    while frames < 200 {
    //        let block = Block::new();
    //        let progress = Paragraph::new(format!("{frames}/200"));
    //        frame.render_widget(progress, area);
    //        app.run_one_iteration().unwrap();
    //        frames = calibrator.process();
    //    }

    //    app.stop_all_tasks().unwrap();

    //    let screen = loader_screen(frame, "Calibrating...");

    //    let model = calibrator.calibrate();
    //}

        pub fn build_cam_calib_view(&mut self, calibration_frames: u64) -> bool {
            let cam_id = Select::new()
                .items(self.camera_configs.keys())
                .default(0)
                .interact()
                .unwrap();
            let dev_id = self.cameras.get(cam_id).unwrap();
            let cam = self.camera_configs.get_mut(dev_id).unwrap();
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
                if let Some(cam) = self.camera_configs.get_mut(dev_id) {
                    cam.calib = Some(CalibratedModel::from_str(
                        serde_json::to_string(&model).unwrap(),
                    ));
                }
            }

            true
        }

        pub fn configure_cam_id(&mut self, camera_index: usize) -> Result<()> {
            let dev_id = self.cameras.get(camera_index).unwrap();

            let cam_id: String = dialoguer::Input::new()
                .with_prompt("Cam ID")
                .interact_text()
                .unwrap();

        let cam_config = self.camera_configs.get_mut(dev_id).unwrap();
        cam_config.cam_id = Some(cam_id.parse()?);

            Ok(())
        }

    pub fn configure_cam_offsets(&mut self, camera_index: usize) -> Result<()> {
        let dev_id = self.cameras.get(camera_index).unwrap();

        println!("Camera offsets");
        let trans_x: String = dialoguer::Input::new()
            .with_prompt(" |- Translation X")
            .interact_text()
            .unwrap();
        let trans_y: String = dialoguer::Input::new()
            .with_prompt(" |- Translation Y")
            .interact_text()
            .unwrap();
        let trans_z: String = dialoguer::Input::new()
            .with_prompt(" |- Translation Z")
            .interact_text()
            .unwrap();
        let rot_w: String = dialoguer::Input::new()
            .with_prompt(" |- Rotation W")
            .interact_text()
            .unwrap();
        let rot_x: String = dialoguer::Input::new()
            .with_prompt(" |- Rotation X")
            .interact_text()
            .unwrap();
        let rot_y: String = dialoguer::Input::new()
            .with_prompt(" |- Rotation Y")
            .interact_text()
            .unwrap();
        let rot_z: String = dialoguer::Input::new()
            .with_prompt(" '- Rotation Z")
            .interact_text()
            .unwrap();

        let offsets = RobotToCamOffset {
            trans_x: trans_x.parse()?,
            trans_y: trans_y.parse()?,
            trans_z: trans_z.parse()?,
            rot_w: rot_w.parse()?,
            rot_x: rot_x.parse()?,
            rot_y: rot_y.parse()?,
            rot_z: rot_z.parse()?,
        };
        let cam_config = self.camera_configs.get_mut(dev_id).unwrap();
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
        let cam_config = self.camera_configs.get_mut(dev_id).unwrap();
        cam_config.width = Some(structure.get::<i32>("width").unwrap() as u32);
        cam_config.height = Some(structure.get::<i32>("height").unwrap() as u32);
    }

    /// Save the configuration to disk
    pub fn save(self) {
        //self.c.validate_logging_config().unwrap();
        //let serialized_config = self.c.serialize_ron();
        //let mut f = OpenOptions::new()
        //    .create(true)
        //    .write(true)
        //    .truncate(true)
        //    .open("chalkydri.ron")
        //    .unwrap();
        //f.write_all(serialized_config.unwrap().as_bytes()).unwrap();
        let config = ConfiguratorConfig {
            cameras: self.camera_configs,
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

use color_eyre::Result;
use indicatif::ProgressBar;
use serde::Deserialize;

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Home,
    Config,
    Caps,
    Calibrator,
}

#[derive(clap::Parser)]
#[command(version, about, long_about = None)]
pub enum Command {
    Configure,
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
            config.build_cam_calib_view(calibration_frames);
            config.save_cuconfig();
            config.save();
        }
    }
    /*
                for (keycode, description) in [
                    ("c", "Calibrate camera"),
                    ("Up/Down", "Select camera"),
                    ("Enter", "Configure camera"),
                    ("q", "Quit"),
                ] {
                    keybind_text.push_span(Span::raw(keycode).bold());
                    keybind_text.push_span([" ", description, "   "].concat());
                }

                let keybind_text = Paragraph::new(keybind_text).block(keybind_block);

                match view {
                    View::Home => {
                        let camera_list = config.build_cam_list().block(main_block);
                        fr.render_widget(camera_list, main);
                    }
                    View::Config => {
                        let config_list = config.build_cam_config_list().block(main_block);
                        fr.render_widget(config_list, main);
                    }
                    View::Caps => {
                        let caps_list = config.build_cam_cap_list(current_cam).block(main_block);
                        fr.render_widget(caps_list, main);
                    }
                    View::Calibrator => {
                        let done = config.build_cam_calib_view(fr, main);
                        if done {
                            view = View::Config;
                        }
                    }
                }
                fr.render_widget(keybind_text, keybinds);
            })?;

            if let Some(key) = event::read()?.as_key_press_event() {
                match key.code {
                    KeyCode::Char('q') => {
                        break 'main_frame_loop;
                    }
                    KeyCode::Char('c') => {
                        let cam_id = config.cameras.get(current_cam).unwrap();
                        config.current_cam = Some(cam_id.clone());
                        view = View::Calibrator;
                    }
                    KeyCode::Up => {
                        config.list_index = config.list_index.saturating_sub(1);
                        config.clamp_list_index();
                    }
                    KeyCode::Down => {
                        config.list_index += 1;
                        config.clamp_list_index();
                    }
                    KeyCode::Enter => match view {
                        View::Home => {
                            current_cam = config.list_index;
                            config.list_index = 0;
                            config.list_len = CAM_CONFIG_OPTS.len();
                            view = View::Config;
                        }
                        View::Config => {
                            view = CAM_CONFIG_OPTS[config.list_index].1;
                            config.list_index = 0;
                        }
                        View::Calibrator => {}
                        View::Caps => {
                            let cam_id = config.cameras.get(current_cam).unwrap();
                            if let Some(ref caps) = config.caps {
                                let cap = caps.get(config.list_index).unwrap();
                                if let Some(cam) = config.camera_configs.get_mut(cam_id) {
                                    cam.width = Some(cap.get::<i32>("width").unwrap() as u32);
                                    cam.height = Some(cap.get::<i32>("height").unwrap() as u32);
                                }
                            }
                            config.list_index = 0;
                            config.caps = None;
                            view = View::Config;
                        }
                    },
                    _ => {}
                }
            }
        }
    */

    Ok(())
}
