use cu_zenoh_bridge::ZenohBridge;
use cu29::prelude::*;

pub mod messages {
    use serde::{Deserialize, Serialize};

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
