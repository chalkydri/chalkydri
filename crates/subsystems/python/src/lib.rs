mod api;

use std::{
    collections::HashMap,
    ffi::CStr,
    ops::Deref,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use chalkydri_core::{preprocs::SubsysPreprocessor, subsystems::SubsysProcessor};
use chalkydri_core::subsystems::Subsystem;
use chalkydri_core::{
    gstreamer::{self, Caps, Element, ElementFactory, prelude::GstBinExtManual},
    tokio::sync::RwLock,
};
use chalkydri_core::{nt_client::publish::GenericPublisher, prelude::*};
use chalkydri_core::{
    nt_client::{data::Properties, publish::Publisher},
    preprocs::frame_proc_loop,
};
use numpy::ndarray;
//use tokio_util::task::TaskTracker;

use chalkydri_core::{Error, config};

use pyo3::prelude::*;

#[derive(Clone)]
pub struct PythonSubsys {
    topics: Arc<RwLock<HashMap<String, GenericPublisher>>>,
    modules: Arc<RwLock<Vec<Box<Py<PyModule>>>>>,
}
impl Subsystem for PythonSubsys {
    const NAME: &'static str = "python";

    type Config = Vec<config::CustomSubsystem>;
    type Preproc = PythonPreproc;
    type Proc = Self;

    async fn init(
        nt: &nt_client::ClientHandle,
        cam_config: config::Camera,
    ) -> Result<Self, <Self::Proc as SubsysProcessor>::Error> {
        let mut topics = Arc::new(RwLock::new(HashMap::new()));
        let mut modules = Arc::new(RwLock::new(Vec::new()));

        for subsys in cam_config.clone().subsystems.custom {
            // Read custom subsystems from the configuration
            let subsystems = Cfg.read().custom_subsystems.clone();
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
                    PyModule::from_code(py, code, file_name, module_name)
                        .unwrap()
                        .into()
                });

                // Save It for Later :)
                modules.write().await.push(Box::new(module));
            }
        }

        Ok::<Self, <Self::Proc as SubsysProcessor>::Error>(PythonSubsys { topics, modules })
    }
}
impl SubsysProcessor for PythonSubsys {
    type Subsys = Self;
    type Output = ();
    type Error = PyErr;

    #[instrument(skip_all, fields(cam_id = cam_config.id))]
    async fn process(
        &self,
        subsys: Self::Subsys,
        nt: &nt_client::ClientHandle,
        cam_config: config::Camera,
        frame: Arc<Vec<u8>>,
    ) -> Result<Self::Output, Self::Error> {
        trace!("a");

        let cam_config = cam_config.clone();
        let modules = self.modules.clone();
        if let Some(settings) = cam_config.settings {
            let settings = settings.clone();
            trace!("wtf");
            let modules = modules.clone();
            let arr = ndarray::Array::from_shape_vec(
                //(settings.height as usize, settings.width as usize, 3usize),
                (1280, 720, 3),
                Arc::try_unwrap(frame).unwrap(),
            )
            .expect("something is really braken");
            let nparr = Python::attach(|py| numpy::PyArray::from_array(py, &arr).unbind());

            for module in modules.read().await.iter() {
                let ret: HashMap<String, f64> = Python::attach(|py| {
                    let module = module.bind(py);
                    trace!("running module");
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
        }

        Ok::<Self::Output, Self::Error>(())
    }
}

#[derive(Clone)]
pub struct PythonPreproc {
    videoconvertscale: Arc<Element>,
    filter: Arc<Element>,
}
impl SubsysPreprocessor for PythonPreproc {
    type Frame = Vec<u8>;
    type Subsys = PythonSubsys;

    fn init(pipeline: &gstreamer::Pipeline) -> Self {
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
