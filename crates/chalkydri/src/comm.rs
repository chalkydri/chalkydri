use std::{collections::HashMap, sync::Arc};

use chalkydri_core::prelude::RwLock;
use cu29::prelude::*;
use whacknet::{RobotPose, VisionUncertainty, WhacknetClient};

#[derive(Clone)]
pub struct Comm {
    clients: Arc<RwLock<HashMap<u8, WhacknetClient>>>,
}
//impl<'c> Comm<'c> {
impl Comm {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    pub fn publish(&self, cam_id: u8, tag_count: u8, ts: u64, pose: RobotPose, std_devs: VisionUncertainty) {
        let mut has_init = true;

        if let Some(clients) = self.clients.try_read() {
            if let Some(client) = clients.get(&cam_id) {
                match client.send(ts, tag_count, pose, std_devs) {
                    Err(err) => {
                        error!("failed to send pose: {err:?}");
                    }
                    _ => {}
                }
            } else {
                has_init = false;
            }
        }

        if !has_init {
            let client = WhacknetClient::new(cam_id).expect("failed to initialize client");
            self.clients.write().insert(cam_id, client);
        }
    }
}

pub struct CommBundle;
bundle_resources!(CommBundle: Comm);

impl ResourceBundle for CommBundle {
    fn build(
        bundle: BundleContext<Self>,
        _config: Option<&ComponentConfig>,
        manager: &mut ResourceManager,
    ) -> CuResult<()> {
        let comm_key = bundle.key(CommBundleId::Comm);

        manager.add_owned(comm_key, Comm::new())?;

        Ok(())
    }
}
