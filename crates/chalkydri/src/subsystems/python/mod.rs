mod api;

use std::{
    collections::HashMap,
    ffi::CStr,
    pin::{pin, Pin},
    sync::Arc,
    task::{Context, Poll},
};

use gstreamer::{prelude::GstBinExtManual, Caps, Element, ElementFactory};
use nt_client::{data::Properties, publish::Publisher};
use numpy::ndarray;
use tokio::sync::{Mutex, RwLock};
use tokio_util::task::TaskTracker;

use crate::{cameras::preproc::Preprocessor, config, error::Error, subsystems::Subsystem, Cfg, Nt};

use pyo3::prelude::*;

use super::frame_proc_loop;

#[derive(Clone)]
pub struct PythonSubsys;
impl Subsystem for PythonSubsys {
    const NAME: &'static str = "python";

    type Error = PyErr;
    type Config = Vec<config::CustomSubsystem>;
    type Preproc = PythonPreproc;
    type Output = ();

    async fn init() -> Result<Self, Self::Error> {
        Ok::<Self, Self::Error>(PythonSubsys)
    }

    #[tracing::instrument(skip_all, fields(cam_id = cam_config.id))]
    async fn process(
        &self,
        nt: &nt_client::ClientHandle,
        cam_config: config::Camera,
        rx: tokio::sync::watch::Receiver<
            Option<
                Arc<<<Self as Subsystem>::Preproc as crate::cameras::preproc::Preprocessor>::Frame>,
            >,
        >,
    ) -> Result<Self::Output, Self::Error> {
        let tt = TaskTracker::new();
        let mut topics = Arc::new(RwLock::new(HashMap::<String, Publisher<f64>>::new()));
        let mut modules = Arc::new(RwLock::new(Vec::new()));

            for camera in Cfg.read()
                .await
                .cameras
                .clone()
                .unwrap()
            {
                for subsys in camera.subsystems.custom {
                    // Read custom subsystems from the configuration
                    let subsystems = futures_executor::block_on(Cfg.read())
                        .custom_subsystems
                        .clone();
                    if let Some(subsys) = subsystems.get(&subsys) {
                        // Add a null terminator to the end of all of these things
                        let code = [subsys.code.as_bytes(), &[0u8]].concat();
                        let file_name = [b"custom_code.py".as_slice(), &[0u8]].concat();
                        let module_name = [b"custom_code".as_slice(), &[0u8]].concat();

                        // Convert them all to CStrs
                        let code = CStr::from_bytes_with_nul(&code).unwrap();
                        let file_name = CStr::from_bytes_with_nul(&file_name).unwrap();
                        let module_name = CStr::from_bytes_with_nul(&module_name).unwrap();

                        // Load the code in
                        let module = Python::attach(|py| -> Py<PyModule> {
                            PyModule::from_code(py, code, file_name, module_name).unwrap().into()
                        });

                        // Save It for Later :)
                        modules.write().await.push(module);
                    }
                }
            }

            let rx = rx.clone();

            trace!("a");
            let tt = tt.clone();

            let cam_config = cam_config.clone();
            let modules = modules.clone();
            if let Some(settings) = cam_config.settings {
                let settings = settings.clone();
                frame_proc_loop::<Self::Preproc, _>(rx.clone(), async move |buf| {
                    let modules = modules.clone();
                        let arr = ndarray::Array::from_shape_vec(
                            //(settings.height as usize, settings.width as usize, 3usize),
                            (1280, 720, 3),
                            buf,
                        )
                        .expect("something is really braken");
                        let nparr = Python::attach(|py| {
                            numpy::PyArray::from_array(py, &arr).unbind()
                        });

                        for module in modules.read().await.iter() {
                            let ret: HashMap<String, f64> = Python::attach(|py| {
                                let module = module.bind(py);
                                println!("running {module}");
                                 module
                                    .getattr("run")
                                    .unwrap()
                                    .call1((nparr.bind(py),))
                                    .unwrap()
                                    .extract()
                                    .unwrap()
                            });

                            for (k, v) in ret {
                                let (k, v) = (k.clone(), v.clone());
                                trace!("{k}: {v}");
                                let topic_name = format!("/chalkydri/subsystems/{k}");


                                let mut topic = Nt
                                    .topic(topic_name.clone())
                                    .publish::<f64>(Properties::default())
                                    .await
                                    .unwrap();
                                topic.set(v).await.unwrap();
                            }
                        }

                        tokio::task::yield_now().await;
                }).await;
            }



            Ok::<Self::Output, Self::Error>(())
    }
}

pub struct PythonPreproc {
    videoconvertscale: Arc<Element>,
    filter: Arc<Element>,
}
impl Preprocessor for PythonPreproc {
    type Frame = Vec<u8>;
    type Subsys = PythonSubsys;

    fn new(pipeline: &gstreamer::Pipeline) -> Self {
        // Create the elements
        let videoconvertscale = ElementFactory::make("videoconvertscale").build().unwrap();
        let filter = ElementFactory::make("capsfilter")
            .property(
                "caps",
                &Caps::builder("video/x-raw").field("format", "BGR").build(),
            )
            .build()
            .unwrap();

        // Add them to the pipeline
        pipeline.add_many([&videoconvertscale, &filter]).unwrap();

        Self {
            videoconvertscale: videoconvertscale.into(),
            filter: filter.into(),
        }
    }
    fn link(&self, src: Element, sink: Element) {
        Element::link_many([&src, &self.videoconvertscale, &self.filter, &sink]).unwrap();
    }
    fn unlink(&self, src: Element, sink: Element) {
        Element::unlink_many([&src, &self.videoconvertscale, &self.filter, &sink]);
    }
    fn sampler(
        appsink: &gstreamer_app::AppSink,
        tx: tokio::sync::watch::Sender<Option<Arc<Self::Frame>>>,
    ) -> Result<Option<()>, Error> {
        let sample = appsink
            .pull_sample()
            .map_err(|_| Error::FailedToPullSample)?;
        let buf = sample.buffer().unwrap();
        let buf = buf
            .to_owned()
            .into_mapped_buffer_readable()
            .unwrap()
            .to_vec();

        tx.send(Some(Arc::new(buf))).unwrap();

        Ok(Some(()))
    }
}

struct AllowThreads<F>(F);

impl<F> std::future::Future for AllowThreads<F>
where
    F: Future + Unpin + Send,
    F::Output: Send,
{
    type Output = F::Output;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let waker = cx.waker();
        Python::attach(|py| {
            py.detach(|| std::pin::pin!(&mut self.0).poll(&mut Context::from_waker(waker)))
        })
    }
}
