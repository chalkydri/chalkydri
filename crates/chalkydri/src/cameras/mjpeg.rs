use std::task::Poll;

use actix_web::web::Bytes;
use futures_core::Stream;
use tokio::sync::watch;

use crate::error::Error;

/// Wrapper over frame buffer receiver
#[derive(Clone)]
pub struct MjpegStream {
    pub(super) rx: watch::Receiver<Option<Vec<u8>>>,
}
impl Stream for MjpegStream {
    type Item = Result<Bytes, Error>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        loop {
            match self.rx.has_changed() {
                Ok(true) => {
                    info!("working!!!");

                    let bytes = if let Some(frame) = self.get_mut().rx.borrow_and_update().as_deref() {
                        [
                            b"--frame\r\nContent-Length: ",
                            frame.len().to_string().as_bytes(),
                            b"\r\nContent-Type: image/jpeg\r\n\r\n",
                            frame,
                        ]
                        .concat()
                    } else {
                        Vec::new()
                    };

                    return Poll::Ready(Some(Ok(bytes.into())));
                }
                Ok(false) => {}
                Err(err) => {
                    error!("error getting frame: {err:?}");

                    return Poll::Ready(None);
                }
            }
        }
    }
}
