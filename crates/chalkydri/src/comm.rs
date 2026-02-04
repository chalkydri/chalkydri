use std::sync::Arc;

use cu_zenoh_bridge::ZenohBridge;
use cu29::prelude::*;
use zenoh::{Session, Wait, bytes::Encoding, pubsub::Publisher};

use crate::comm::messages::Position;
use zenoh_ext::{Deserialize, Serialize, z_serialize};

pub mod messages {
    use super::*;
    use std::io::Write;

    #[derive(Debug, Default, Clone)]
    pub struct Ping {
        pub seq: u64,
        pub note: String,
    }

    /// `chalkydri/gyro/{name}`
    #[derive(Debug, Default, Clone)]
    pub struct Gyro {
        /// Robot's heading/rotation
        pub rot: f64,
    }

    /// `chalkydri/coproc/{name}/position`
    ///
    /// Position is relative to blue origin
    #[derive(Debug, Default, Clone)]
    pub struct Position {
        /// X coord
        pub x: f64,
        /// Y coord
        pub y: f64,
        /// Rotation
        pub rot: f64,
        /// Confidence (0-255)
        pub confidence: u8,
        /// Timestamp
        pub ts: u32,
    }
    impl Serialize for Position {
        fn serialize(&self, serializer: &mut zenoh_ext::ZSerializer) {
            serializer.serialize(self.x);
            serializer.serialize(self.y);
            serializer.serialize(self.rot);
            serializer.serialize(self.confidence);
            serializer.serialize(self.ts);
        }
    }
}

#[derive(Clone)]
pub struct Comm {
    dev_name: String,
    session: Session,
    //publisher: Arc<Publisher<'c>>,
}
//impl<'c> Comm<'c> {
impl Comm {
    pub fn new(dev_name: impl Into<String>) -> Self {
        let mut cfgg = zenoh::Config::default();
        let session = zenoh::open(cfgg).wait().unwrap();

        //let publisher = session
        //    .declare_publisher(format!("chalkydri/coproc/{}", dev_name.into()))
        //    .wait()
        //    .unwrap();

        Self {
            dev_name: dev_name.into(),
            session,
            //publisher: Arc::new(publisher),
        }
    }
    pub fn publish(&self, pos: Position) {
        self.session
            .put(
                format!("chalkydri/coproc/{}", self.dev_name),
                z_serialize(&pos),
            )
            .encoding(Encoding::ZENOH_SERIALIZED)
            .wait()
            .unwrap();
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

        manager.add_owned(comm_key, Comm::new("test"))?;

        Ok(())
    }
}
