use parking_lot::{RwLock, const_rwlock};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Write},
    path::Path,
    sync::LazyLock,
};

use crate::Error;

#[allow(non_upper_case_globals)]
pub static Cfg: LazyLock<RwLock<Config>> = LazyLock::new(|| const_rwlock(Config::default()));

macro_rules! def_cfg {
    ($(
        $struct_ident:ident {
            $(
            $(# [ $attr:ident $( ( $tt:tt ) )* ])?
            $ident:ident : $ty:ty ,
            )*
        }
    )*) => {
       $(
           #[derive(Deserialize, Serialize, Debug, Clone)]
           #[cfg_attr(feature = "__openapi", derive(utoipa::ToSchema))]
           pub struct $struct_ident {
               $(
                $(#[$attr $( ($tt) )?])?
                pub $ident: $ty,
               )*
           }
       )*
    };
}

def_cfg! {
    Config {
        team_number: u16,
        ntables_ip: Option<String>,
        rerun: Option<Rerun>,
        cameras: Option<Vec<Camera>>,
        device_name: Option<String>,

        field_layout: Option<String>,
        field_layouts: Option<HashMap<String, serde_json::Value>>,

        custom_subsystems: HashMap<String, CustomSubsystem>,
    }
    Rerun {
        server_address: Option<String>,
    }
    Camera {
        #[serde(skip_deserializing)]
        online: bool,
        id: String,
        name: String,
        settings: Option<CameraSettings>,
        //#[serde(skip_deserializing)]
        possible_settings: Option<Vec<CameraSettings>>,
        subsystems: CameraSubsystems,
        calib: Option<String>,
        auto_exposure: bool,
        manual_exposure: Option<u32>,
        orientation: VideoOrientation,
        cam_offsets: CameraOffsets,
    }
    CameraSettings {
        width: u32,
        height: u32,
        frame_rate: Option<CfgFraction>,
        format: Option<String>,
    }
    CfgFraction {
        num: u32,
        den: u32,
    }
    CameraOffsets {
        translation: CameraOffsetDimensions,
        rotation: CameraOffsetDimensions,
    }
    CameraOffsetDimensions {
        x: f64,
        y: f64,
        z: f64,
    }
    CameraSubsystems {
        mjpeg: Option<MjpegSubsys>,
        capriltags: Option<CAprilTagsSubsys>,
        ml: Option<MlSubsys>,
        custom: Vec<String>,
    }
    MjpegSubsys {
        width: u32,
        height: u32,
    }
    CAprilTagsSubsys {
        max_frame_rate: u8,
    }
    MlSubsys {
    }
    CustomSubsystem {
        code: String,
    }
}

impl Config {
    /// Load the configuration from the specified path
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Error> {
        let mut f = File::open(path).map_err(|_| Error::FailedToReadConfig)?;
        let mut buf = String::new();
        f.read_to_string(&mut buf).unwrap();
        toml::from_str(&buf).map_err(|_| Error::InvalidConfig)
    }

    /// Save the configuration to the specified path
    pub async fn save(&self, path: impl AsRef<Path>) -> Result<(), Error> {
        let mut f = File::create(path).unwrap();
        let toml_cfgg = toml::to_string_pretty(&self).unwrap();
        f.write_all(toml_cfgg.as_bytes()).unwrap();
        f.flush().unwrap();

        Ok(())
    }
}
impl Default for Config {
    fn default() -> Self {
        Self {
            team_number: u16::MAX,
            ntables_ip: None,
            rerun: None,
            cameras: None,
            device_name: None,

            field_layout: None,
            field_layouts: None,
            custom_subsystems: HashMap::new(),
        }
    }
}
impl Default for Camera {
    fn default() -> Self {
        Self {
            online: false,
            id: String::new(),
            name: String::new(),
            settings: None,
            auto_exposure: true,
            manual_exposure: None,
            possible_settings: None,
            subsystems: CameraSubsystems {
                mjpeg: Some(MjpegSubsys {
                    width: 1280,
                    height: 720,
                }),
                capriltags: Some(CAprilTagsSubsys { max_frame_rate: 40 }),
                ml: None,
                custom: Vec::new(),
            },
            calib: None,
            orientation: VideoOrientation::None,
            cam_offsets: CameraOffsets {
                translation: CameraOffsetDimensions {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                rotation: CameraOffsetDimensions {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
            },
        }
    }
}

impl Default for CameraSettings {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            frame_rate: None,
            format: None,
        }
    }
}

#[derive(Deserialize, Serialize, Clone)]
#[cfg_attr(feature = "__openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum CameraKind {
    PiCam,
    Usb,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[cfg_attr(feature = "__openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "kebab-case")]
pub enum VideoOrientation {
    None = 0,
    Clockwise = 1,
    #[serde(rename = "rotate-180")]
    Rotate180 = 2,
    Counterclockwise = 3,
}
