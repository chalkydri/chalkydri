use std::{sync::Arc, time::Duration};

use sophus_autodiff::{linalg::VecF64, prelude::*};
use sophus_lie::{Isometry3F64, Rotation3F64, prelude::*};
use tokio::sync::{Mutex, mpsc};

use crate::{Cfg, Nt, error::Error};

/// Keeps pose transforms and ...
#[derive(Clone)]
pub struct PoseEstimator {
    ///// Transform registry
    //reg: Arc<Mutex<Registry>>,
    //tx: mpsc::Sender<Transform>,
}
impl PoseEstimator {
    pub async fn new() -> Result<Self, Error> {
        //let reg = Arc::new(Mutex::new(Registry::new(Duration::from_secs(90))));
        //let (tx, mut rx) = mpsc::channel(64);

        //let reg_ = reg.clone();
        //tokio::spawn(async move {
        //    while let Some(transform) = rx.recv().await {
        //        reg_.lock().await.add_transform(transform);
        //    }
        //});

        //Ok(Self { reg, tx })
        Ok(Self {})
    }

    /// Add a transform to the transform registry
    pub async fn add_transform(
        &self,
        tag_est_pos: Isometry3F64,
        tag_field_pos: Isometry3F64,
    ) -> Result<(), Error> {
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

        Ok(())
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
            }
        });
    }
}
