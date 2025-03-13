use crate::{error::Error, subsys::capriltags::AprilTagFieldLayout};
use std::{collections::HashMap, fs::File, io::Read, path::Path};

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
    }
    CameraSettings {
        width: u32,
        height: u32,
        frame_rate: CfgFraction,
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
    }
    MlSubsys {
        enabled: bool,
    }
}

impl Config {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Error> {
        let mut f = File::open(path).map_err(|_| Error::FailedToReadConfig)?;
        let mut buf = String::new();
        f.read_to_string(&mut buf).unwrap();
        toml::from_str(&buf).map_err(|_| Error::InvalidConfig)
    }
}

#[derive(Deserialize, Serialize, Clone)]
#[cfg_attr(feature = "web", derive(utopia::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum CameraKind {
    PiCam,
    Usb,
}
