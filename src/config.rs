use crate::error::Error;
use std::{fs::File, io::Read, path::Path};

#[derive(Deserialize, Serialize, Clone)]
#[cfg_attr(feature = "web", derive(utopia::ToSchema))]
pub struct Config {
    pub team_number: u16,
    pub ntables_ip: Option<String>,
    //pub version: String,
    pub rerun: Option<RerunConfig>,
    pub cameras: Vec<CameraConfig>,
    //pub camera: HashMap<String, CameraConfig>,
    //pub tpu: Option<TpuConfig>,
    //pub backends: HashMap<Backend, BackendConfig>,
}
impl Config {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Error> {
        let mut f = File::open(path).map_err(|_| Error::FailedToReadConfig)?;
        let mut buf = String::new();
        f.read_to_string(&mut buf).unwrap();
        println!("{}", &buf);
        toml::from_str(&buf).map_err(|_| Error::InvalidConfig)
    }
}

#[derive(Deserialize, Serialize, Clone)]
#[cfg_attr(feature = "web", derive(utopia::ToSchema))]
pub struct RerunConfig {
    pub server_address: Option<String>,
}

#[derive(Deserialize, Serialize, Clone)]
#[cfg_attr(feature = "web", derive(utopia::ToSchema))]
pub struct CameraConfig {
    pub name: String,
    pub settings: Option<CameraSettings>,
    pub caps: Vec<CameraSettings>,
}

#[derive(Deserialize, Serialize, Clone)]
#[cfg_attr(feature = "web", derive(utopia::ToSchema))]
pub struct CameraSettings {
    pub width: u32,
    pub height: u32,
    pub frame_rate: CfgFraction,
    pub gamma: Option<f32>,
}

#[derive(Deserialize, Serialize, Clone)]
#[cfg_attr(feature = "web", derive(utopia::ToSchema))]
pub struct CfgFraction {
    pub num: u32,
    pub den: u32,
}

#[derive(Deserialize, Serialize, Clone)]
#[cfg_attr(feature = "web", derive(utopia::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum CameraKind {
    PiCam,
    Usb,
}

#[derive(Deserialize, Serialize, Clone)]
#[cfg_attr(feature = "web", derive(utopia::ToSchema))]
pub struct TpuConfig {
    //pub kind: tfledge::CoralDeviceKind,
}
