use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use chalkydri::cameras::pipeline::CamPipeline;
use chalkydri::cameras::providers::{CamProvider, CamProviderBundle, PROVIDER, V4l2Provider};
use chalkydri::cameras::GstToCuImage;
use cu29::bincode::config::Config;
use cu29::config::{ComponentConfig, CuConfig, CuGraph, Node};
use cu29::prelude::*;
use cu29_helpers::basic_copper_setup;
use gstreamer::prelude::{DeviceExt, ElementExt, PadExt};
use gstreamer::{State, Structure};

mod calibration;
use calibration::*;

#[copper_runtime(config = "../../config/calibration.ron")]
struct App {}

const CAM_CONFIG_OPTS: &'static [(&'static str, View)] = &[
    (
        "Set camera resolution / frame rate / pixel format",
        View::Caps,
    ),
    ("<- Back to camera list", View::Home),
];

#[derive(Default)]
pub struct CamSettings {
    cam_id: Option<u8>,
    width: Option<u32>,
    height: Option<u32>,
    calib: Option<CalibratedModel>,
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
    caps: Option<Vec<Structure>>,
    list_index: usize,
    list_len: usize,
    provider: Option<V4l2Provider>,
    calibrator: Option<Calibrator>,
    calib_frames: usize,
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

        let provider = Some(PROVIDER.clone());

        Self {
            has_run: false,
            c,
            camera_configs: HashMap::new(),
            cameras: Vec::new(),
            current_cam: None,
            list_index: 0,
            provider,
            caps: None,
            list_len: 0,
            calibrator: None,
            calib_frames: 0,
        }
    }

    pub fn find_cameras(&mut self) {
        if let Some(ref mut provider) = self.provider {
            provider.start();
        }
        std::thread::sleep(Duration::from_secs(2));
    }

    pub fn configure_cam(&mut self) {
        let dev_id = self.current_cam.clone().unwrap();
        let curr_cam = self.camera_configs.get(&dev_id).unwrap();
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

    fn clamp_list_index(&mut self) {
        if self.list_index >= self.list_len {
            self.list_index = self.list_len - 1;
        }
    }

    pub fn refresh_cameras(&mut self) {
        self.camera_configs.clear();
        self.cameras.clear();

        if let Some(ref provider) = self.provider {
            for device in provider.devices() {
                self.camera_configs
                    .insert(device.clone(), Default::default());
                self.cameras.push(device);
            }
        }
    }

    pub fn build_cam_list(&self) -> List {
        let index = self.list_index;

        let list = List::new(self.cameras.iter().enumerate().map(|(i, cam)| {
            let mut item = ListItem::new(cam.clone());
            if index == i {
                item = item.bold().blue();
            }
            item
        }));

        list
    }

    pub fn build_cam_config_list(&self) -> List {
        let index = self.list_index;
        List::new(
            CAM_CONFIG_OPTS
                .into_iter()
                .enumerate()
                .map(|(i, (opt, _))| {
                    let mut item = ListItem::new(*opt);
                    if index == i {
                        item = item.bold().blue();
                    }
                    item
                }),
        )
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
    pub fn build_cam_calib_view(&mut self, frame: &mut Frame, area: Rect) -> bool {
        let dev_id = self.current_cam.clone().unwrap();
        let cam = self.camera_configs.get_mut(&dev_id).unwrap();
        let width = cam.width.unwrap();
        let height = cam.height.unwrap();

        // Initialize on first call
        if self.calibrator.is_none() {
            if let Some(ref mut provider) = self.provider.take() {
                provider.stop();
            }
            let calibrator = Calibrator::new();

            let pathbuf = PathBuf::from_str("chalkydri.copper").unwrap();
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

            self.calibrator = Some(calibrator);
            self.calib_frames = 0;
        }

        // Render progress
        let progress = Paragraph::new(format!("{}/200", self.calib_frames)).centered();
        let block = Block::bordered().title("Calibrating...");
        frame.render_widget(progress.block(block), area);

        // Run one iteration and process frame
        std::thread::sleep(Duration::from_millis(10));
        self.calib_frames = self.calibrator.as_mut().unwrap().process();

        // Check if done
        if self.calib_frames >= 200 {
            let model = self.calibrator.as_mut().unwrap().calibrate();

            if let Some(model) = model {
                if let Some(cam) = self.camera_configs.get_mut(&dev_id) {
                    cam.calib = Some(CalibratedModel::from_str(
                        serde_json::to_string(&model).unwrap(),
                    ));
                }
            }

            // Cleanup
            self.calibrator = None;
            self.calib_frames = 0;

            self.provider = Some(V4l2Provider::init());
            return true;
        }

        false
    }

    pub fn build_cam_cap_list(&mut self, camera_index: usize) -> List {
        let index = self.list_index;
        let mut list_len = 0;

        let caps = if let Some(ref caps) = self.caps {
            caps.to_owned()
        } else {
            if let Some(ref provider) = self.provider {
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
                self.caps = Some(caps.clone());
                caps
            } else {
                panic!("oopsie");
            }
        };

        let list = List::new(
            caps.iter()
                .filter_map(|structure| {
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
                })
                .enumerate()
                .map(|(i, text)| {
                    let mut item = ListItem::new(text);
                    if index == i {
                        item = item.bold().blue();
                    }
                    list_len += 1;
                    item
                }),
        );

        self.list_len = list_len;

        list
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

fn app<T: ratatui::backend::Backend>(t: &mut Terminal<T>) -> Result<()> {
    let mut config = Configurator::new();

    t.draw(|fr| loader_screen(fr, "Initializing...")).unwrap();

    // Initialize GStreamer
    match gstreamer::init() {
        Ok(()) => {}
        Err(e) => {
            panic!("gstreamer failed to initialize: {e:?}");
        }
    }

    let provider = V4l2Provider::init();
    t.draw(|fr| loader_screen(fr, "Finding cameras..."))
        .unwrap();
    provider.start();

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
        config.configure_cam();
    }

    t.draw(|fr| loader_screen(fr, "Saving configuration..."))
        .unwrap();
    config.save();

    t.draw(|fr| loader_screen(fr, "Syncing disk...")).unwrap();
    rustix::fs::sync();

    Ok(())
}

use color_eyre::Result;
use crossterm::event::{self, KeyCode};
use ratatui::layout::{Layout, Rect};
use ratatui::prelude::CrosstermBackend;
use ratatui::style::Stylize;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Bar, List, ListItem, Padding};
use ratatui::Terminal;
use ratatui::{
    layout::Constraint,
    widgets::{Block, Clear, Paragraph},
    Frame,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Home,
    Config,
    Caps,
    Calibrator,
}

fn main() -> Result<()> {
    color_eyre::install()?;

    ratatui::run(|t| {
        t.draw(|fr| loader_screen(fr, "Initializing...")).unwrap();

        // Initialize GStreamer
        match gstreamer::init() {
            Ok(()) => {}
            Err(e) => {
                panic!("gstreamer failed to initialize: {e:?}");
            }
        }

        let mut config = Configurator::new();
        let mut current_cam = 0usize;
        let mut view = View::Home;

        t.draw(|fr| loader_screen(fr, "Finding cameras..."))
            .unwrap();
        config.find_cameras();
        config.refresh_cameras();

        'main_frame_loop: loop {
            t.draw(|fr| {
                let area = fr.area();

                let layout =
                    Layout::vertical([Constraint::Fill(1), Constraint::Length(3)]).spacing(0);
                let [main, keybinds] = area.layout(&layout);

                let main_block = Block::bordered().padding(Padding::symmetric(2, 0));
                let keybind_block = Block::bordered();
                let mut keybind_text = Text::raw("  ");
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

        Ok(())
    })
}

fn loader_screen(frame: &mut Frame, msg: &'static str) {
    modal_screen(
        frame,
        msg,
        Some(concat!(
            "  Chalkydri Configurator v",
            env!("CARGO_PKG_VERSION"),
            "  "
        )),
    )
}
fn modal_screen(frame: &mut Frame, msg: &'static str, title: Option<&'static str>) {
    let area = frame.area();

    let mut block = Block::bordered();
    if let Some(title) = title {
        block = block.title(title);
    }
    let centered_area = area.centered(Constraint::Length(60), Constraint::Length(5));
    frame.render_widget(Clear, centered_area);
    let msg_ui = Paragraph::new(["\n", msg].concat()).centered().block(block);
    frame.render_widget(msg_ui, centered_area);
}
