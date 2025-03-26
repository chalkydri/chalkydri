mod api;

use std::{any::Any, collections::HashMap, ffi::CStr};

use gstreamer::{
    Caps, Element, ElementFactory,
    prelude::{GstBinExt, GstBinExtManual},
};
use numpy::{
    PyArrayMethods,
    ndarray::{self, ShapeBuilder},
};
use pyo3::{ffi::c_str, types::PyDict};

use crate::{Cfg, config, error::Error, subsystems::Subsystem};

use pyo3::prelude::*;

use super::frame_proc_loop;

#[derive(Clone)]
pub struct PythonSubsys;
impl Subsystem for PythonSubsys {
    const NAME: &'static str = "python";

    type Error = Error;
    type Config = Vec<config::CustomSubsys>;
    type Output = ();

    async fn init() -> Result<Self, Self::Error> {
        Ok::<Self, Self::Error>(PythonSubsys)
    }
    async fn process(
        &self,
        manager: super::SubsysManager,
        nt: minint::NtConn,
        cam_config: crate::config::Camera,
        rx: tokio::sync::watch::Receiver<Option<Vec<u8>>>,
    ) -> Result<Self::Output, Self::Error> {
        Python::with_gil(|py| -> PyResult<()> {
            let mut modules = Vec::new();

            for camera in Cfg.blocking_read().cameras.clone().unwrap() {
                for subsys in camera.subsystems.custom {
                    let code = [subsys.code.as_bytes(), &[0u8]].concat();
                    let file_name = [b"custom_code.py".as_slice(), &[0u8]].concat();
                    let module_name = [b"custom_code".as_slice(), &[0u8]].concat();
                    let code = CStr::from_bytes_with_nul(&code).unwrap();
                    let file_name = CStr::from_bytes_with_nul(&file_name).unwrap();
                    let module_name = CStr::from_bytes_with_nul(&module_name).unwrap();

                    let module = PyModule::from_code(py, code, file_name, module_name).unwrap();
                    let module = module.unbind();

                    modules.push(module);
                }
            }

            py.allow_threads(move || {
                futures_executor::block_on(frame_proc_loop(rx, async move |buf| {
                    if let Some(settings) = &cam_config.settings {
                        Python::with_gil(|py| -> PyResult<()> {
                            let arr = ndarray::Array2::from_shape_vec(
                                (settings.width as usize, settings.height as usize)
                                    .strides((settings.width as usize, 1usize)),
                                buf,
                            )
                            .unwrap();
                            let nparr = numpy::PyArray2::from_array(py, &arr);

                            for module in &modules {
                                let ret: HashMap<String, f64> = module
                                    .getattr(py, "run")
                                    .unwrap()
                                    .call1(py, (nparr.clone(),))
                                    .unwrap()
                                    .extract(py)
                                    .unwrap();
                                debug!("{ret:?}");
                            }

                            Ok(())
                        });
                    }
                }));
            });

            Ok(())
        });

        Ok::<Self::Output, Self::Error>(())
    }
    fn preproc(
        config: crate::config::Camera,
        pipeline: &gstreamer::Pipeline,
    ) -> Result<(gstreamer::Element, gstreamer::Element), Self::Error> {
        // Create the elements
        let videoconvertscale = ElementFactory::make("videoconvertscale").build().unwrap();
        let filter = ElementFactory::make("capsfilter")
            .property(
                "caps",
                &Caps::builder("video/x-raw")
                    .field("width", &1280)
                    .field("height", &720)
                    .field("format", "RGB")
                    .build(),
            )
            .build()
            .unwrap();

        // Add them to the pipeline
        pipeline.add_many([&videoconvertscale, &filter]).unwrap();

        // Link them
        Element::link_many([&videoconvertscale, &filter]).unwrap();

        Ok((videoconvertscale, filter))
    }
}
