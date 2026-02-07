extern crate cu_bincode as bincode;

use serde::{Serialize, Deserialize};
use bytemuck::{Pod, Zeroable};
use bincode::{Decode, Encode};
use std::{io, net::UdpSocket, sync::Arc};

const BIND_ADDR: &str = "0.0.0.0:0";
const REMOTE_ADDR: &str = "10.45.33.2:7001";

// The acutal positioning data from the code
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Pod, Zeroable, Encode, Decode, Serialize, Deserialize)]
pub struct RobotPose {
    /// X coord
    pub x: f64,
    /// Y coord
    pub y: f64,
    /// Rotation
    pub rot: f64,
}

/// Freaky stuff that the addVisionMeasurment wants in the code. Nathan please calculate.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Pod, Zeroable)]
pub struct VisionUncertainty {
    /// Standard deviation of X in meters
    pub x: f64,
    /// Standard deviation of Y in meters
    pub y: f64,
    /// Standard deviation of Rotation in radians
    pub rot: f64,
}

/// This is what gets sent over the wire to rio. 64 bytes, just like minecraft...
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Pod, Zeroable)]
struct VisionMeasurement {
    /// Our estimated robot pose
    pub pose: RobotPose, // 24 bytes
    /// Our accurracy stdevs to send to the bot
    pub std_devs: VisionUncertainty, // 24 bytes
    /// Camera id
    camera_id: u64,
    /// Timestamp (in micro secs)
    ts: u64,
}

pub struct WhacknetClient {
    cam_id: u64,
    socket: Arc<UdpSocket>,
}
impl WhacknetClient {
    /// Initialize a new whacknet client
    pub fn new(cam_id: u64) -> io::Result<Self> {
        // Create and connect to server
        let socket = UdpSocket::bind(BIND_ADDR)?;
        socket.connect(REMOTE_ADDR)?;

        Ok(Self {
            cam_id,
            socket: Arc::new(socket),
        })
    }
    /// Send a pose with std dev
    pub fn send(&self, ts: u64, pose: RobotPose, std_devs: VisionUncertainty) -> io::Result<()> {
        // Pack up all the data in the struct
        let measurement = VisionMeasurement {
            pose,
            std_devs,
            camera_id: self.cam_id,
            ts,
        };

        // Turn the measurement into raw bytes and send it over the UDP sock
        let bytes = bytemuck::bytes_of(&measurement);
        self.socket.send(bytes)?;

        Ok(())
    }
}

#[test]
fn check_size() {
    assert_eq!(std::mem::size_of::<VisionMeasurement>(), 64);
}

// TODO: add a benchmark
