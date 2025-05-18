use std::{collections::HashMap, sync::Arc, time::Duration};

use field_layout::{AprilTagFieldLayout, Field};
use sophus_autodiff::{linalg::VecF64, prelude::*};
use sophus_lie::{HasAverage, Isometry3F64, Rotation3F64, prelude::*};
use tokio::sync::{Mutex, RwLock, mpsc};

use crate::{Cfg, Nt, error::Error};

pub(crate) mod field_layout;

/// Keeps pose transforms and ...
#[derive(Clone)]
pub struct PoseEstimator {
    ///// Transform registry
    //reg: Arc<Mutex<Registry>>,
    //tx: mpsc::Sender<Transform>,
    layout: Arc<RwLock<Option<AprilTagFieldLayout>>>,
    tag_mappings: Arc<RwLock<Option<HashMap<usize, Isometry3F64>>>>,
    poses_rx: Arc<Mutex<mpsc::UnboundedReceiver<Isometry3F64>>>,
    poses_tx: mpsc::UnboundedSender<Isometry3F64>,
}
impl PoseEstimator {
    pub async fn new() -> Result<Self, Error> {
        let (poses_tx, poses_rx) = mpsc::unbounded_channel();

        let est = Self {
            layout: Arc::new(RwLock::new(None)),
            tag_mappings: Arc::new(RwLock::new(None)),
            poses_tx,
            poses_rx: Arc::new(Mutex::new(poses_rx)),
        };

        est.load_layout().await?;

        Ok(est)
    }

    /// (Re)load the field layout
    pub async fn load_layout(&self) -> Result<(), Error> {
        if let Some(layouts) = &Cfg.read().await.field_layouts {
            if let Some(layout_name) = &Cfg.read().await.field_layout {
                if let Some(layout) = layouts.get(layout_name) {
                    *self.layout.write().await = Some(layout.clone());

                    Ok(())
                } else {
                    Err(Error::FieldLayoutDoesNotExist)
                }
            } else {
                Err(Error::FieldLayoutNotSelected)
            }
        } else {
            Err(Error::NoFieldLayouts)
        }
    }

    /// Add a transform to the transform registry
    pub async fn add_transform_from_tag(
        &self,
        tag_est_pos: Isometry3F64,
        tag_id: usize,
    ) -> Result<(), Error> {
        if let Some(tag_mappings) = self.tag_mappings.read().await.clone() {
            if let Some(tag_field_pos) = tag_mappings.get(&tag_id) {
                let cam_est_rel_pos = tag_est_pos.inverse();
                let cam_relto_pos = cam_est_rel_pos.translation() / 2.0;

                let cam_relto_x = -cam_relto_pos.x;
                let cam_relto_y = cam_relto_pos.y;
                let cam_relto_z = -cam_relto_pos.z;

                let cam_angle = Rotation3F64::try_from_mat(
                    tag_field_pos.rotation().matrix() - cam_est_rel_pos.rotation().matrix(),
                )
                .unwrap();

                let cam_fcs_abs = Isometry3F64::from_translation_and_rotation(
                    VecF64::<3>::new(
                        cam_relto_x + tag_field_pos.translation().x,
                        cam_relto_y + tag_field_pos.translation().y,
                        cam_relto_z + tag_field_pos.translation().z,
                    ),
                    cam_angle,
                );

                debug!("{cam_fcs_abs}");

                //let cam_fcs_abs_x = cam_fcs_abs.translation().x;
                //let cam_fcs_abs_y = cam_fcs_abs.translation().y;
                //let cam_fcs_abs_z = cam_fcs_abs.translation().z;

                //let robot_angle = cam_fcs_abs.rotation() +

                //
                self.poses_tx.send(cam_fcs_abs).unwrap();

                Ok(())
            } else {
                Err(Error::InvalidTag)
            }
        } else {
            Err(Error::FieldLayoutNotSelected)
        }
    }

    ///// Interpolate the robot's pose based on transforms
    //pub async fn get_robot_pose(&self) -> Result<Transform, Error> {
    //    match self
    //        .reg
    //        .lock()
    //        .await
    //        .get_transform("robot", "field", Timestamp::now())
    //    {
    //        Ok(t) => Ok(t),
    //        Err(err) => Err(Error::FailedToGetPose(err)),
    //    }
    //}

    pub async fn nt_loop(&self) {
        let est = self.clone();
        tokio::spawn(async move {
            let mut t = Nt
                .publish::<Vec<f64>>(format!(
                    "/chalkydri/robot_pose/{}/translation",
                    Cfg.read().await.device_name.clone().unwrap()
                ))
                .await
                .unwrap();
            let mut r = Nt
                .publish::<Vec<f64>>(format!(
                    "/chalkydri/robot_pose/{}/rotation",
                    Cfg.read().await.device_name.clone().unwrap()
                ))
                .await
                .unwrap();
            loop {
                //match est.get_robot_pose().await {
                //    Ok(pose) => {
                //        t.set(vec![
                //            pose.translation.x,
                //            pose.translation.y,
                //            pose.translation.z,
                //        ])
                //        .await;
                //        r.set(vec![
                //            pose.rotation.w,
                //            pose.rotation.x,
                //            pose.rotation.y,
                //            pose.rotation.z,
                //        ])
                //        .await;
                //    }
                //    Err(err) => {
                //        error!("failed to get pose");
                //    }
                //}

                let mut poses = Vec::new();
                while let Some(pose) = est.poses_rx.lock().await.recv().await {
                    poses.push(pose);
                }
                let pose = Isometry3F64::average(&poses).unwrap();
                t.set(vec![
                    pose.translation().x,
                    pose.translation().y,
                    pose.translation().z,
                ])
                .await;
                let rot = pose.rotation().matrix().as_slice().to_vec();
                r.set(rot).await;

                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        });
    }
}
