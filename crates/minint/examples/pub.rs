extern crate minint;

use std::time::Duration;

use minint::{NtConn, NtError};

#[tokio::main]
async fn main() -> Result<(), NtError> {
    env_logger::init();
    let conn = NtConn::new("127.0.0.1", "minint-test").await?;

    let mut test = conn.publish::<String>("/test").await.unwrap();

    let mut i = 0;
    loop {
        test.set(format!("{i}")).await.unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;
        i += 1;
    }
}
