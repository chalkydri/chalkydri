extern crate cu_bincode as bincode;

use bincode::{Decode, Encode};
use bytemuck::{Pod, Zeroable};
use chalkydri_core::parking_lot;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::mpsc;
use std::{io, net::UdpSocket, sync::Arc};

use chalkydri_core::prelude::{Mutex, RwLock};
use cu29::prelude::*;

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
    /// Timestamp (in micro secs)
    ts: u64,
    /// Camera id
    camera_id: u8,
    /// Tag count
    tag_count: u8,
    /// Reserved for future use
    _reserved_1: u8,
    /// Reserved for future use
    _reserved_2: u8,
    /// Reserved for future use
    _reserved_3: u8,
    /// Reserved for future use
    _reserved_4: u8,
    /// Reserved for future use
    _reserved_5: u8,
    /// Reserved for future use
    _reserved_6: u8,
}

pub struct WhacknetClient {
    socket: Arc<UdpSocket>,
}
impl WhacknetClient {
    /// Initialize a new whacknet client
    pub fn new() -> io::Result<Self> {
        // Create and connect to server
        let socket = UdpSocket::bind(BIND_ADDR)?;
        socket.connect(REMOTE_ADDR)?;

        Ok(Self {
            socket: Arc::new(socket),
        })
    }
    /// Send a pose with std dev
    pub fn send(&self, measurement: VisionMeasurement) -> io::Result<()> {
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

#[derive(Clone)]
pub struct Comm {
    clients: Arc<RwLock<HashMap<u8, WhacknetClient>>>,
    gyro_angle: Arc<RwLock<Option<f64>>>,
    measurements_tx: Arc<mpsc::Sender<VisionMeasurement>>,
}
impl Comm {
    /// Initialize the communication handler thingie
    pub fn new() -> Self {
        let gyro_angle = Arc::new(RwLock::new(Some(0f64)));

        // Just putting the gyro value listener on its own thread
        let gyro_angle_ = gyro_angle.clone();
        std::thread::spawn(move || {
            let gyro_socket = UdpSocket::bind("0.0.0.0:7002").unwrap();

            let mut buf = [0u8; 8];
            loop {
                match gyro_socket.recv(&mut buf) {
                    Ok(_bytes) => {
                        let mut guard = gyro_angle_.write();
                        if guard.is_none() {
                            break;
                        }
                        *guard = Some(f64::from_le_bytes(buf));
                    }
                    Err(_err) => {}
                }

                buf = [0u8; 8];
            }
        });

        let (tx, rx) = mpsc::channel();
        let measurements_tx = Arc::new(tx);

        std::thread::spawn(move || {
            let client = WhacknetClient::new().expect("failed to initialize client");
            loop {
                while let Ok(measurement) = rx.recv() {
                    client.send(measurement).unwrap();
                }
            }
        });

        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            gyro_angle,
            measurements_tx,
        }
    }

    /// Send a pose estimate to the RIO
    pub fn publish(
        &self,
        cam_id: u8,
        tag_count: u8,
        ts: u64,
        pose: RobotPose,
        std_devs: VisionUncertainty,
    ) {
        // Pack up all the data in the struct
        let measurement = VisionMeasurement {
            pose,
            std_devs,
            camera_id: cam_id,
            tag_count,
            ts,
            ..Default::default()
        };

        self.measurements_tx.send(measurement).unwrap();
    }

    /// Get the robot's heading from the gyro
    pub fn gyro_angle(&self) -> Option<f64> {
        self.gyro_angle
            .try_read()
            .map(|ga| ga.expect("this should not be possible"))
    }
}
impl Drop for Comm {
    fn drop(&mut self) {
        // Tells the gyro listener thread to exit
        *self.gyro_angle.write() = None;
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
