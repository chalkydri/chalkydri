use cu29::prelude::*;
use cu_zenoh_bridge::ZenohBridge;

pub mod messages {
    use serde::{Serialize, Deserialize};

    #[derive(Debug, Default, Clone, Serialize, Deserialize)]
    pub struct Ping {
        pub seq: u64,
        pub note: String,
    }

    #[derive(Debug, Default, Clone, Serialize, Deserialize)]
    pub struct Position {
        pub reply: String,
    }
}
