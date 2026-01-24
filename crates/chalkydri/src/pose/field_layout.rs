use std::collections::HashMap;

use nalgebra as na;
use sophus_autodiff::linalg::VecF64;
use sophus_lie::{Isometry3F64, Rotation3F64};

use crate::error::Error;

use super::PoseEstimator;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "web", derive(utopia::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct AprilTagFieldLayout {
    pub tags: Vec<LayoutTag>,
    pub field: Field,
}
impl AprilTagFieldLayout {
    pub async fn load(
        &self,
        pose_est: &mut PoseEstimator,
    ) -> Result<HashMap<usize, na::Isometry3<f64>>, Error> {
        let mut tags: HashMap<usize, na::Isometry3<f64>> = HashMap::new();
        for LayoutTag {
            id,
            pose:
                LayoutPose {
                    translation,
                    rotation: LayoutRotation { quaternion },
                },
        } in self.tags.clone()
        {
            // Turn the field layout values into Rust datatypes
            let translation = na::Translation3::new(translation.x, translation.y, translation.z);
            let rotation =
                na::Quaternion::new(quaternion.x, quaternion.y, quaternion.z, quaternion.w);
            let rotation = na::UnitQuaternion::from_quaternion(rotation);
            let isometry = na::Isometry3::from_parts(translation, rotation);

            tags.insert(id as usize, isometry);
        }

        Ok(tags)
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "web", derive(utopia::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LayoutTag {
    #[serde(rename = "ID")]
    pub id: i64,
    pub pose: LayoutPose,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "web", derive(utopia::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LayoutPose {
    pub translation: LayoutTranslation,
    pub rotation: LayoutRotation,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "web", derive(utopia::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LayoutTranslation {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "web", derive(utopia::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LayoutRotation {
    pub quaternion: LayoutQuaternion,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "web", derive(utopia::ToSchema))]
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
#[cfg_attr(feature = "web", derive(utopia::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct Field {
    pub length: f64,
    pub width: f64,
}
