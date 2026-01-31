use cu29::prelude::*;
use cu_zenoh_bridge::ZenohBridge;
use zenoh::{bytes::Encoding, pubsub::Publisher, Session, Wait};

use crate::comm::messages::Position;
use zenoh_ext::{z_serialize, Deserialize, Serialize};

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

pub struct Comm<'c> {
    session: Session,
    publisher: Publisher<'c>,
}
impl Comm<'_> {
    pub async fn new(dev_name: impl Into<String>) -> Self {
        let session = zenoh::open(zenoh::Config::default()).wait().unwrap();

        let publisher = session
            .declare_publisher(format!("chalkydri/coproc/{}", dev_name.into()))
            .wait()
            .unwrap();

        Self {
            session,
            publisher,
        }
    }
    pub fn publish(&self) {
        self.publisher
            .put(z_serialize(&Position::default()))
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
