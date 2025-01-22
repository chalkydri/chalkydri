use std::collections::HashMap;

#[derive(Deserialize, Serialize)]
pub struct Config {
    pub sigma: String,
    pub version: String,
    pub camera: HashMap<String, CameraConfig>,
    pub tpu: Option<TpuConfig>,
    //pub backends: HashMap<Backend, BackendConfig>,
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
