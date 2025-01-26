use crate::error::Error;
use std::{collections::HashMap, fs::File, path::Path};

#[derive(Deserialize, Serialize)]
pub struct Config {
    pub team_number: u16,
    pub version: String,
    pub camera: HashMap<String, CameraConfig>,
    pub tpu: Option<TpuConfig>,
    //pub backends: HashMap<Backend, BackendConfig>,
}
impl Config {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Error> {
        let f = File::open(path).map_err(|_| Error::FailedToReadConfig)?;
        serde_json::from_reader(f).map_err(|_| Error::InvalidConfig)
    }
}

#[derive(Deserialize, Serialize)]
pub struct CameraConfig {
    pub kind: CameraKind,
    pub id: Option<String>,
    pub resolution: Option<CameraResolution>,
}

#[derive(Deserialize, Serialize)]
pub struct CameraResolution {
    pub width: u32,
    pub height: u32,
}

#[derive(Deserialize, Serialize)]
pub enum CameraKind {
    PiCam,
    Usb,
}

#[derive(Deserialize, Serialize)]
pub struct TpuConfig {
    //pub kind: tfledge::CoralDeviceKind,
}
