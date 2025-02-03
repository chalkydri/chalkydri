use std::path::Path;

use camera_intrinsic_model::{self as model, GenericModel};

use model::model_from_json;

pub struct CalibratedModel {
    model: GenericModel<f64>,
}
impl CalibratedModel {
    pub fn new() -> Self {
        let mut path = Path::new("/boot/cam0.json");
        if !path.exists() {
            path = Path::new("./cam0.json");
        }

        // Load the camera model
        let model = model_from_json(path.to_str().unwrap());

        Self { model }
    }

    pub const fn inner_model(&self) -> GenericModel<f64> {
        self.model
    }
}
