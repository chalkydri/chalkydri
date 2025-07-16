mod api;

use std::{collections::HashMap, ffi::CStr, sync::Arc};

use gstreamer::{Caps, Element, ElementFactory, prelude::GstBinExtManual};
use minint::NtTopic;
use numpy::ndarray;
use tokio::sync::RwLock;

use crate::{
    Cfg, Nt, cameras::pipeline::Preprocessor, config, error::Error, subsystems::Subsystem,
};

use pyo3::prelude::*;

use super::frame_proc_loop;

#[derive(Clone)]
pub struct PythonSubsys;
impl Subsystem for PythonSubsys {
    const NAME: &'static str = "python";

    type Error = Error;
    type Config = Vec<config::CustomSubsystem>;
    type Preproc = PythonPreproc;
    type Output = ();

    async fn init() -> Result<Self, Self::Error> {
        Ok::<Self, Self::Error>(PythonSubsys)
    }

    async fn process(
        &self,
        nt: minint::NtConn,
        cam_config: config::Camera,
        rx: tokio::sync::watch::Receiver<
            Option<<<Self as Subsystem>::Preproc as Preprocessor>::Frame>,
        >,
    ) -> Result<Self::Output, Self::Error> {
        let handle = tokio::runtime::Handle::current();

        Python::with_gil(|py| -> PyResult<()> {
            let mut modules = Vec::new();

            for camera in futures_executor::block_on(Cfg.read())
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
                        let module = PyModule::from_code(py, code, file_name, module_name).unwrap();
                        // Unbind the module from Python's GIL
                        let module = module.unbind();

                        // Save It for Later :)
                        modules.push(module);
                    }
                }
            }

            py.allow_threads(move || {
                let handle_ = handle.clone();
                handle.spawn(async move {
                    let topics = Arc::new(RwLock::new(HashMap::<String, NtTopic<f64>>::new()));
                    frame_proc_loop::<Self::Preproc, _>(rx, async move |buf| {
                        if let Some(settings) = &cam_config.settings {
                            let py_ret = Python::with_gil(|py| -> PyResult<()> {
                                let nparr = numpy::PyArray::from_array(py, &buf);

                                for module in &modules {
                                    let ret: HashMap<String, f64> = module
                                        .getattr(py, "run")?
                                        .call1(py, (nparr.clone(),))?
                                        .extract(py)?;

                                    for (k, v) in ret {
                                        let (k, v) = (k.clone(), v.clone());
                                        let topics = topics.clone();
                                        handle_.spawn(async move {
                                            let topic_name = format!("/chalkydri/subsystems/{k}");

                                            let mut topics = topics.write().await;

                                            if let Some(topic) = topics.get_mut(&k) {
                                                topic.set(v).await.unwrap();
                                            } else {
                                                let mut topic = Nt
                                                    .publish::<f64>(topic_name.clone())
                                                    .await
                                                    .unwrap();
                                                topic.set(v).await.unwrap();
                                                topics.insert(topic_name, topic);
                                            }
                                        });
                                    }
                                }

                                Ok(())
                            });

                            if let Err(err) = py_ret {
                                error!("{err}");
                            }
                        }
                    })
                    .await;
                });
            });

            Ok(())
        })
        .unwrap();

        Ok::<Self::Output, Self::Error>(())
    }
}

pub struct PythonPreproc {
    videoconvertscale: Arc<Element>,
    filter: Arc<Element>,
}
impl Preprocessor for PythonPreproc {
    type Frame = ndarray::Array3<u8>;
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

        let arr = ndarray::Array::from_shape_vec(
            //(settings.height as usize, settings.width as usize, 3usize),
            (1280, 720, 3),
            buf,
        )
        .expect("something is really braken");

        tx.send(Arc::new(Some(arr))).unwrap();

        Ok(Some(()))
    }
}
