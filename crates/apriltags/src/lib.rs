#[cfg(windows)]
compile_error!(
    "this does not work under windows. please use a unix system. only linux is supported."
);

const SIGN_FLIP_CONST: f64 = 600.0;

#[macro_use]
extern crate serde;
extern crate chalkydri_sqpnp;
extern crate cu_bincode as bincode;
extern crate serde_json;

mod field_layout;

use std::collections::HashMap;
use std::mem::ManuallyDrop;

use apriltag::{Detector, DetectorBuilder, Family, Image, TagParams};

use apriltag_sys::image_u8_t;

use bincode::de::Decoder;
use bincode::error::DecodeError;
use bincode::{Decode, Encode};
use camera_intrinsic_model::{GenericModel, OpenCVModel5};
use chalkydri_sqpnp::{Rot3, SqPnP, Vec3};
use cu_sensor_payloads::CuImage;
use cu_spatial_payloads::Pose as CuPose;
use cu29::prelude::*;
use nalgebra::{Matrix, Matrix2x1, Vector2, Vector3, matrix};
use serde::ser::SerializeTuple;
use serde::{Deserialize, Deserializer, Serialize};

use chalkydri_sqpnp::{Iso3, Pnt3};
use whacknet::{Comm, CommBundleId, RobotPose, VisionUncertainty};

use crate::field_layout::AprilTagFieldLayout;

// the maximum number of detections that can be returned by the detector
const MAX_DETECTIONS: usize = 16;

// Defaults
const TAG_SIZE: f64 = 0.14;
const FX: f64 = 2600.0;
const FY: f64 = 2600.0;
const CX: f64 = 900.0;
const CY: f64 = 520.0;
const FAMILY: &str = "tag36h11";

#[derive(Default, Debug, Clone, Encode)]
pub struct AprilTagDetections {
    pub ids: CuArrayVec<usize, MAX_DETECTIONS>,
    pub poses: CuArrayVec<CuPose<f32>, MAX_DETECTIONS>,
    pub decision_margins: CuArrayVec<f32, MAX_DETECTIONS>,
}

impl Decode<()> for AprilTagDetections {
    fn decode<D: Decoder<Context = ()>>(decoder: &mut D) -> Result<Self, DecodeError> {
        let ids = CuArrayVec::<usize, MAX_DETECTIONS>::decode(decoder)?;
        let poses = CuArrayVec::<CuPose<f32>, MAX_DETECTIONS>::decode(decoder)?;
        let decision_margins = CuArrayVec::<f32, MAX_DETECTIONS>::decode(decoder)?;
        Ok(AprilTagDetections {
            ids,
            poses,
            decision_margins,
        })
    }
}

// implement serde support for AprilTagDetections
// This is so it can be logged with debug!.
impl Serialize for AprilTagDetections {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let CuArrayVec(ids) = &self.ids;
        let CuArrayVec(poses) = &self.poses;
        let CuArrayVec(decision_margins) = &self.decision_margins;
        let mut tup = serializer.serialize_tuple(ids.len())?;

        ids.iter()
            .zip(poses.iter())
            .zip(decision_margins.iter())
            .map(|((id, pose), margin)| (id, pose, margin))
            .for_each(|(id, pose, margin)| {
                tup.serialize_element(&(id, pose, margin)).unwrap();
            });

        tup.end()
    }
}

impl<'de> Deserialize<'de> for AprilTagDetections {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct AprilTagDetectionsVisitor;

        impl<'de> serde::de::Visitor<'de> for AprilTagDetectionsVisitor {
            type Value = AprilTagDetections;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a tuple of (id, pose, decision_margin)")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut detections = AprilTagDetections::new();
                while let Some((id, pose, decision_margin)) = seq.next_element()? {
                    let CuArrayVec(ids) = &mut detections.ids;
                    ids.push(id);
                    let CuArrayVec(poses) = &mut detections.poses;
                    poses.push(pose);
                    let CuArrayVec(decision_margins) = &mut detections.decision_margins;
                    decision_margins.push(decision_margin);
                }
                Ok(detections)
            }
        }

        deserializer.deserialize_tuple(MAX_DETECTIONS, AprilTagDetectionsVisitor)
    }
}

impl AprilTagDetections {
    fn new() -> Self {
        Self::default()
    }
    pub fn filtered_by_decision_margin(
        &self,
        threshold: f32,
    ) -> impl Iterator<Item = (usize, &CuPose<f32>, f32)> {
        let CuArrayVec(ids) = &self.ids;
        let CuArrayVec(poses) = &self.poses;
        let CuArrayVec(decision_margins) = &self.decision_margins;

        ids.iter()
            .zip(poses.iter())
            .zip(decision_margins.iter())
            .filter_map(move |((id, pose), margin)| {
                (*margin > threshold).then_some((*id, pose, *margin))
            })
    }
}

pub struct Resources<'r> {
    pub comm: Borrowed<'r, Comm>,
}
impl<'r> ResourceBindings<'r> for Resources<'r> {
    type Binding = CommBundleId;
    fn from_bindings(
        manager: &'r mut ResourceManager,
        mapping: Option<&ResourceBindingMap<Self::Binding>>,
    ) -> CuResult<Self> {
        let key = mapping
            .expect("comm binding")
            .get(Self::Binding::Comm)
            .expect("comm")
            .typed();
        Ok(Self {
            comm: manager.borrow(key)?,
        })
    }
}

#[derive(Reflect)]
#[reflect(from_reflect = false)]
pub struct AprilTags {
    #[reflect(ignore)]
    detector: Detector,
    #[reflect(ignore)]
    solver: SqPnP,
    #[reflect(ignore)]
    tags: HashMap<usize, Iso3>,
    #[reflect(ignore)]
    comm: Comm,
    #[reflect(ignore)]
    cam_model: GenericModel<f64>,
    last_time: Option<u64>,
    cam_id: u8,
    //#[reflect(ignore)]
    //robot_to_cam: Option<Iso3>,
}

#[derive(Default, Serialize, Deserialize)]
pub struct RobotToCamOffset {
    pub rot_w: f64,
    pub rot_x: f64,
    pub rot_y: f64,
    pub rot_z: f64,
    pub trans_x: f64,
    pub trans_y: f64,
    pub trans_z: f64,
}

fn image_from_cuimage<A>(cu_image: &CuImage<A>) -> ManuallyDrop<Image>
where
    A: ArrayLike<Element = u8>,
{
    unsafe {
        // Try to emulate what the C code is doing on the heap to avoid double free
        let buffer_ptr = cu_image.buffer_handle.with_inner(|inner| inner.as_ptr());
        let low_level_img = Box::new(image_u8_t {
            buf: buffer_ptr as *mut u8,
            width: cu_image.format.width as i32,
            height: cu_image.format.height as i32,
            stride: cu_image.format.stride as i32,
        });
        let ptr = Box::into_raw(low_level_img);
        ManuallyDrop::new(Image::from_raw(ptr))
    }
}

impl Freezable for AprilTags {}

impl CuSinkTask for AprilTags {
    type Input<'m> = input_msg!((CuImage<Vec<u8>>, CuDuration));
    type Resources<'r> = Resources<'r>;

    fn new(_config: Option<&ComponentConfig>, resources: Self::Resources<'_>) -> CuResult<Self>
    where
        Self: Sized,
    {
        let comm = resources.comm.0.clone();

        if let Some(config) = _config {
            let family_cfg: String = config.get("family").unwrap().unwrap_or(FAMILY.to_string());
            let family: Family = family_cfg.parse().unwrap();
            let bits_corrected: u32 = config.get("bits_corrected").unwrap().unwrap_or(3);
            let tagsize = config.get("tag_size").unwrap().unwrap_or(TAG_SIZE);
            let cam_id: u8 = config.get("cam_id").unwrap().unwrap();
            //let robot_to_cam_str: String = config.get("robot_to_cam").unwrap().unwrap();
            //let fx = config.get("fx").unwrap_or(FX);
            //let fy = config.get("fy").unwrap_or(FY);
            //let cx = config.get("cx").unwrap_or(CX);
            //let cy = config.get("cy").unwrap_or(CY);
            //let field_layout_path = config.get("field_json_path");
            let calib = config.get::<String>("calib").unwrap().unwrap();

            //let robot_to_cam_offsets: RobotToCamOffset = serde_json::from_str(&robot_to_cam_str).unwrap();
            //let translation = nalgebra::Translation3::new(robot_to_cam_offsets.trans_x, robot_to_cam_offsets.trans_y, robot_to_cam_offsets.trans_z);
            //let rotation =
            //    nalgebra::Quaternion::new(robot_to_cam_offsets.rot_w, robot_to_cam_offsets.rot_x, robot_to_cam_offsets.rot_y, robot_to_cam_offsets.rot_z);
            //let rotation = nalgebra::UnitQuaternion::from_quaternion(rotation);
            //let robot_to_cam = Iso3::from_parts(translation, rotation);

            let cam_model: GenericModel<f64> = serde_json::from_str(&calib).unwrap();

            let detector = DetectorBuilder::default()
                .add_family_bits(family, bits_corrected as usize)
                .build()
                .unwrap();

            let solver = SqPnP::new();

            return Ok(Self {
                cam_id,
                detector,
                solver,
                tags: AprilTagFieldLayout::load().unwrap(),
                comm,
                cam_model,
                last_time: None,
                //robot_to_cam: Some(robot_to_cam),
            });
        }
        Ok(Self {
            cam_id: u8::MAX,
            detector: DetectorBuilder::default()
                .add_family_bits(FAMILY.parse::<Family>().unwrap(), 1)
                .build()
                .unwrap(),
            solver: SqPnP::new(),
            tags: AprilTagFieldLayout::load().unwrap(),
            comm,
            cam_model: GenericModel::OpenCVModel5(OpenCVModel5::zeros()),
            last_time: None,
            //robot_to_cam: None,
        })
    }

    fn process<'i>(&mut self, clock: &RobotClock, input: &Self::Input<'i>) -> CuResult<()> {
        let Tov::Time(time) = input.tov() else {
            return Ok(());
        };
        if let Some(payload) = input.payload() {
            use chalkydri_sqpnp::Vec3;

            let image = image_from_cuimage(&payload.0);
            let detections = self.detector.detect(&image);
            if detections.len() > 0 {
                let mut camera_pts: Vec<Vec3> = Vec::new();
                let mut world_pts: Vec<Iso3> = Vec::new();
                let mut sqpnp_buffer: Vec<Pnt3> = Vec::new(); //doing this kinda defeats the point, fix later
                'det_proc: for detection in detections.iter() {
                    let Some(tag) = self.tags.get(&detection.id()) else {
                        continue 'det_proc;
                    };

                    let corners = detection
                        .corners()
                        .into_iter()
                        .map(|corner| Vector2::new(corner[0], corner[1]))
                        .collect::<Vec<_>>();

                    let unprojected = self
                        .cam_model
                        .unproject(corners.as_slice())
                        .into_iter()
                        .filter_map(|corner| corner)
                        .collect::<Vec<_>>();

                    // Only use it if the corners could be unprojected
                    if unprojected.len() == 4 {
                        world_pts.push(tag.clone());
                        camera_pts.extend_from_slice(unprojected.as_slice()); //I didn't check, make sure these are normalized
                    }
                }

                if let Some((cam_to_world_rotation, cam_to_world_translation, std_dev)) =
                    self.solver.solve_robot_pose(
                        &world_pts,
                        &camera_pts,
                        self.comm.gyro_angle().unwrap_or(0.0),
                        SIGN_FLIP_CONST,
                        &mut sqpnp_buffer,
                    )
                {
                    let pose = RobotPose {
                        x: cam_to_world_translation[0],
                        y: cam_to_world_translation[1],
                        rot: cam_to_world_rotation.euler_angles().2,
                    };
                    let uncertainty = VisionUncertainty {
                        x: std_dev[0],
                        y: std_dev[1],
                        rot: std_dev[2],
                    };
                    dbg!(pose);

                    let ts = clock.now().as_micros() - time.as_micros();
                    self.comm.publish(
                        self.cam_id,
                        detections.len().try_into().unwrap_or(u8::MAX),
                        ts,
                        pose.clone(),
                        uncertainty.clone(),
                    );
                }
            } else {
                let timey_time = clock.now().as_millis();
                let ts = clock.now().as_micros() - time.as_micros();
                if self.last_time.is_none() || (timey_time - self.last_time.unwrap()) > 5 {
                    self.comm.publish(
                        self.cam_id,
                        0,
                        ts,
                        RobotPose::default(),
                        VisionUncertainty::default(),
                    );
                    self.last_time = Some(timey_time);
                }
            }
            println!("{time:?}");
        }

        Ok(())
    }
}

#[derive(Reflect)]
pub struct ApriltagsProcessor {}
impl Freezable for ApriltagsProcessor {}
impl CuSinkTask for ApriltagsProcessor {
    type Input<'m> = input_msg!('m, AprilTagDetections);
    type Resources<'r> = ();

    fn new(_config: Option<&ComponentConfig>, _resources: Self::Resources<'_>) -> CuResult<Self>
    where
        Self: Sized,
    {
        Ok(Self {})
    }

    fn start(&mut self, _clock: &RobotClock) -> CuResult<()> {
        Ok(())
    }

    fn process<'i>(&mut self, _clock: &RobotClock, input: &Self::Input<'i>) -> CuResult<()> {
        let input: &AprilTagDetections = input.payload().unwrap();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;
    use anyhow::Context;
    use anyhow::Result;
    use image::{ImageBuffer, ImageReader};
    use image::{Luma, imageops::FilterType, imageops::crop, imageops::resize};

    use cu_sensor_payloads::CuImageBufferFormat;

    #[allow(dead_code)]
    fn process_image(path: &str) -> Result<ImageBuffer<Luma<u8>, Vec<u8>>> {
        let reader = ImageReader::open(path).with_context(|| "Failed to open image")?;
        let mut img = reader
            .decode()
            .context("Failed to decode image")?
            .into_luma8();
        let (orig_w, orig_h) = img.dimensions();

        let new_h = (orig_w as f32 * 9.0 / 16.0) as u32;
        let crop_y = (orig_h - new_h) / 2; // Center crop

        let cropped = crop(&mut img, 0, crop_y, orig_w, new_h).to_image();
        Ok(resize(&cropped, 1920, 1080, FilterType::Lanczos3))
    }

    #[test]
    fn test_end2end_apriltag() -> Result<()> {
        let img = process_image("tests/data/simple.png")?;
        let format = CuImageBufferFormat {
            width: img.width(),
            height: img.height(),
            stride: img.width(),
            pixel_format: "GRAY".as_bytes().try_into()?,
        };
        let buffer_handle = CuHandle::new_detached(img.into_raw());
        let cuimage = CuImage::new(format, buffer_handle);

        let mut config = ComponentConfig::default();
        config.set("tag_size", 0.14);
        config.set("fx", 2600.0);
        config.set("fy", 2600.0);
        config.set("cx", 900.0);
        config.set("cy", 520.0);
        config.set("family", "tag16h5".to_string());

        let mut task = AprilTags::new(Some(&config), ())?;
        let input = CuMsg::<CuImage<Vec<u8>>>::new(Some(cuimage));
        let mut output = CuMsg::<AprilTagDetections>::default();

        let clock = RobotClock::new();
        let result = task.process(&clock, &input, &mut output);
        assert!(result.is_ok());

        if let Some(detections) = output.payload() {
            let detections = detections
                .filtered_by_decision_margin(150.0)
                .collect::<Vec<_>>();

            assert_eq!(detections.len(), 1);
            assert_eq!(detections[0].0, 4);
            return Ok(());
        }
        Err(anyhow::anyhow!("No output"))
    }
}
