use crate::{error::Error, subsys::capriltags::AprilTagFieldLayout};
use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Write},
    path::Path,
};

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
           #[cfg_attr(feature = "web", derive(utopia::ToSchema))]
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
        field_layouts: Option<HashMap<String, AprilTagFieldLayout>>,
    }
    Rerun {
        server_address: Option<String>,
    }
    Camera {
        id: String,
        name: String,
        settings: Option<CameraSettings>,
        //#[serde(skip_deserializing)]
        possible_settings: Option<Vec<CameraSettings>>,
        subsystems: Subsystems,
        calib: Option<serde_json::Value>,
        auto_exposure: bool,
        manual_exposure: Option<u32>,
        orientation: VideoOrientation,
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
    Subsystems {
        capriltags: CAprilTagsSubsys,
        ml: MlSubsys,
    }
    CAprilTagsSubsys {
        enabled: bool,
        gamma: Option<f64>,
        field_layout: Option<String>,
        max_frame_rate: u8,
    }
    MlSubsys {
        enabled: bool,
    }
    CustomSubsys {
        name: String,
        enabled: bool,
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
impl Default for Camera {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            settings: None,
            auto_exposure: true,
            manual_exposure: None,
            possible_settings: None,
            subsystems: Subsystems {
                capriltags: CAprilTagsSubsys {
                    enabled: false,
                    field_layout: None,
                    gamma: None,
                    max_frame_rate: 40,
                },
                ml: MlSubsys { enabled: false },
            },
            calib: None,
            orientation: VideoOrientation::None,
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
#[cfg_attr(feature = "web", derive(utopia::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum CameraKind {
    PiCam,
    Usb,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[cfg_attr(feature = "web", derive(utopia::ToSchema))]
#[serde(rename_all = "kebab-case")]
pub enum VideoOrientation {
    None = 0,
    Clockwise = 1,
    #[serde(rename = "rotate-180")]
    Rotate180 = 2,
    Counterclockwise = 3,
}
