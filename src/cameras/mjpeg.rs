use std::task::Poll;

use actix_web::web::Bytes;
use futures_core::Stream;
use gstreamer::Buffer;
use tokio::sync::watch;

use crate::error::Error;

/// Wrapper over frame buffer receiver
#[derive(Clone)]
pub struct MjpegStream {
    pub(super) rx: watch::Receiver<Option<Buffer>>,
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
                    let mut bytes = Vec::new();
                    bytes.clear();
                    if let Some(frame) = self.get_mut().rx.borrow_and_update().as_deref() {
                        bytes.extend_from_slice(
                            &[
                                b"--frame\r\nContent-Length: ",
                                frame.size().to_string().as_bytes(),
                                b"\r\nContent-Type: image/jpeg\r\n\r\n",
                            ]
                            .concat(),
                        );
                        bytes.extend_from_slice(
                            frame
                                .map_readable()
                                .map_err(|_| Error::FailedToMapBuffer)?
                                .as_slice(),
                        );
                    }

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

