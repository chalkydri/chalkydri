use crate::error::Error;
use std::{collections::HashMap, fs::File, io::Read, path::Path};

#[derive(Deserialize, Serialize)]
pub struct Config {
    pub team_number: u16,
    //pub version: String,
    pub rerun: Option<RerunConfig>,
    pub camera: HashMap<String, CameraConfig>,
    //pub tpu: Option<TpuConfig>,
    //pub backends: HashMap<Backend, BackendConfig>,
}
impl Config {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Error> {
        let mut f = File::open(path).map_err(|_| Error::FailedToReadConfig)?;
        let mut buf = String::new();
        f.read_to_string(&mut buf).unwrap();
        toml::from_str(&buf).map_err(|_| Error::InvalidConfig)
    }
}


#[derive(Deserialize, Serialize)]
pub struct RerunConfig {
    pub server_address: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub struct CameraConfig {
    pub kind: CameraKind,
    pub id: Option<usize>,
    pub resolution: Option<CameraResolution>,
}

#[derive(Deserialize, Serialize)]
pub struct CameraResolution {
    pub width: u32,
    pub height: u32,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CameraKind {
    PiCam,
    Usb,
}

#[derive(Deserialize, Serialize)]
pub struct TpuConfig {
    //pub kind: tfledge::CoralDeviceKind,
}
