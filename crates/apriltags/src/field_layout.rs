use std::{collections::HashMap, fs::File};

use chalkydri_sqpnp::Iso3;
use nalgebra as na;

use chalkydri_core::prelude::*;

//use super::PoseEstimator;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AprilTagFieldLayout {
    pub tags: Vec<LayoutTag>,
    pub field: Field,
}
impl AprilTagFieldLayout {
    /// Attempt to load field layout from field.json
    pub fn load() -> Result<HashMap<usize, Iso3>, ()> {
        let mut f = File::open("field.json").expect("field.json must exist");

        let layout: AprilTagFieldLayout = serde_json::from_reader(&mut f).unwrap();

        let mut tags: HashMap<usize, Iso3> = HashMap::new();
        for LayoutTag {
            id,
            pose:
                LayoutPose {
                    translation,
                    rotation: LayoutRotation { quaternion },
                },
        } in layout.tags.clone()
        {
            // Turn the field layout values into Rust datatypes
            let translation = na::Translation3::new(translation.x, translation.y, translation.z);
            let rotation =
                na::Quaternion::new(quaternion.x, quaternion.y, quaternion.z, quaternion.w);
            let rotation = na::UnitQuaternion::from_quaternion(rotation);
            let isometry = Iso3::from_parts(translation, rotation);

            tags.insert(id as usize, isometry);
        }

        Ok(tags)
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutTag {
    #[serde(rename = "ID")]
    pub id: i64,
    pub pose: LayoutPose,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutPose {
    pub translation: LayoutTranslation,
    pub rotation: LayoutRotation,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutTranslation {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutRotation {
    pub quaternion: LayoutQuaternion,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutQuaternion {
    #[serde(rename = "W")]
    pub w: f64,
    #[serde(rename = "X")]
    pub x: f64,
    #[serde(rename = "Y")]
    pub y: f64,
    #[serde(rename = "Z")]
    pub z: f64,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Field {
    pub length: f64,
    pub width: f64,
}
