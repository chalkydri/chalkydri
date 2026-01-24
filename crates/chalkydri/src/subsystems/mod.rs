use chalkydri_core::{prelude::*, subsystems::SubsystemCtrl};
use futures_util::StreamExt;
use futures_util::TryStreamExt;
use gstreamer::{Element, Pipeline};
use kornia_image::{Image, ImageSize};
use std::{fmt::Debug, marker::PhantomData, sync::Arc, thread::Thread, time::Duration};
use tokio::sync::oneshot;
use tokio::task::LocalSet;
use tokio_util::task::TaskTracker;

use nt_client::ClientHandle as NTClientHandle;
use tokio::sync::{mpsc, watch};

use crate::{cameras::preproc::PreprocWrap, config};

#[cfg(feature = "apriltags")]
pub mod apriltags;
pub mod calibration;
#[cfg(feature = "capriltags")]
pub mod capriltags;
mod manager;
#[cfg(feature = "python")]
pub use chalkydri_subsys_python as python;

pub use manager::SubsysManager;

#[derive(Clone)]
pub struct SubsysRunner<P: SubsysPreprocessor<Frame = Vec<u8>>, S: Subsystem<Preproc = P>> {
    tx: mpsc::Sender<SubsystemCtrl>,
    rx: Arc<Mutex<mpsc::Receiver<SubsystemCtrl>>>,
    preproc: Arc<PreprocWrap<P>>,
    jh: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
    _marker: PhantomData<S>,
}
impl<P: SubsysPreprocessor<Frame = Vec<u8>>, S: Subsystem<Preproc = P>> SubsysRunner<P, S> {
    pub async fn init(
        pipeline: Pipeline,
        cam_config: crate::config::Camera,
        src: Element,
        tt: TaskTracker,
    ) -> Self {
        let (tx, rx) = mpsc::channel(20);

        let preproc = PreprocWrap::<P>::new(&pipeline);

        Self {
            preproc: Arc::new(preproc),
            jh: Arc::new(Mutex::new(None)),
            tx,
            rx: Arc::new(Mutex::new(rx)),
            _marker: PhantomData,
        }
    }

    pub async fn start(&self, cam_config: crate::config::Camera, src: Element, tt: TaskTracker) {
        self.preproc.as_ref().link(src);
        let mut preproc_rx = self.preproc.rx();
        let rx = self.rx.clone();

        *self.jh.lock() = Some(std::thread::spawn(move || {
            let rt = tokio::runtime::LocalRuntime::new().unwrap();
            let _enter = rt.enter();

            let (kill_tx, mut kill_rx) = mpsc::unbounded_channel::<()>();

            rt.spawn_local(tt.track_future(async move {
                //let mut cam_config = cam_config;

                let mut subsys = S::init(Nt.handle(), cam_config.clone()).await.unwrap();
                'preproc_loop: loop {
                    tokio::select! {
                        Some(sample) = preproc_rx.next() => {
                            trace!("processing sample");
                            let buf = sample.buffer().unwrap();
                            let buf = buf
                                .to_owned()
                                .into_mapped_buffer_readable()
                                .unwrap()
                                .to_vec();
                            // subsys
                            //     .process(Nt.handle(), cam_config.clone(), buf.into())
                            //     .await
                            //     .unwrap();
                        }
                        _ = kill_rx.recv() => {
                            //subsys.stop();
                            break 'preproc_loop;
                        }
                    };
                }

                error!("should not exit this early");
            }));

            rt.block_on(async move {
                'msg_loop: while let Some(msg) = rx.lock().recv().await {
                    match msg {
                        SubsystemCtrl::Stop => {
                            kill_tx.send(()).unwrap();
                            break 'msg_loop;
                        }
                        _ => unimplemented!(),
                    }
                }
            });
        }));
    }

    #[instrument(skip(self))]
    pub async fn stop(&self) {
        trace!("sending stop msg");
        self.tx.send(SubsystemCtrl::Stop).await.unwrap();
        trace!("joining thread");
        (*self.jh.lock()).take().unwrap().join().unwrap();
    }
}
