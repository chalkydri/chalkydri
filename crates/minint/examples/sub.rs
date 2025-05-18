extern crate minint;

use std::time::Duration;

use minint::{NtConn, NtError};

#[tokio::main]
async fn main() -> Result<(), NtError> {
    env_logger::init();
    let conn = NtConn::new("10.45.33.2", "minint-test").await?;

    let cam_modes = conn
        .subscribe("/CameraPublisher/Back Left/modes")
        .await
        .unwrap();

    loop {
        if let Some(cam_modes) = cam_modes.get().await.unwrap() {
            dbg!(cam_modes);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
