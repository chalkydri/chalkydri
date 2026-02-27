use std::sync::{Arc, LazyLock};

use chalkydri_core::prelude::Mutex;
use cu29::{
    bundle_resources,
    cutask::{CuMsgPayload, CuSrcTask, Freezable},
    prelude::ResourceBundle,
};
use tokio::sync::mpsc;

use gstreamer::{
    Bus, BusSyncReply, Device, DeviceProvider, DeviceProviderFactory, Message, MessageView,
    Structure, prelude::*,
};

pub static PROVIDER: LazyLock<Arc<Mutex<V4l2Provider>>> = LazyLock::new(|| {
    let prov = V4l2Provider::init();
    prov.register_handler();
    Arc::new(Mutex::new(prov))
});

/// An event from a camera provider
#[derive(Clone, Debug)] //, Default)]
pub(crate) enum ProviderEvent {
    //#[default]
    //Null,
    Connected(String, Device),
    Disconnected(String, Device),
}

pub trait CamProvider {
    fn init() -> Self
    where
        Self: Sized;

    /// Get a unique ID for the given device
    fn get_id(dev: &Device) -> String;

    fn get_by_id(&self, id: String) -> Option<Device>;

    fn inner(&self) -> &DeviceProvider;

    //fn register_handler(&self, tx: mpsc::Sender<ProviderEvent>) {
    //    self.inner()
    //        .bus()
    //        .set_sync_handler(move |_bus: &Bus, msg: &Message| {
    //            match msg.view() {
    //                MessageView::DeviceAdded(msg) => {
    //                    let dev = msg.device();
    //                    let id = Self::get_id(&dev);

    //                    tx.blocking_send(ProviderEvent::Connected(id, dev)).unwrap();
    //                }
    //                MessageView::DeviceRemoved(msg) => {
    //                    let dev = msg.device();
    //                    let id = Self::get_id(&dev);

    //                    tx.blocking_send(ProviderEvent::Disconnected(id, dev))
    //                        .unwrap();
    //                }
    //                _ => unimplemented!(),
    //            }

    //            BusSyncReply::Pass
    //        });
    //}

    fn unregister_handler(&self) {
        self.inner().bus().unset_sync_handler();
    }

    fn start(&self) {
        if !self.inner().is_started() {
            self.inner().start().unwrap();
        }
    }

    fn stop(&self) {
        self.inner().stop();
    }
}

#[derive(Clone)]
pub struct V4l2Provider {
    inner: DeviceProvider,
    tx: mpsc::Sender<ProviderEvent>,
    rx: Arc<Mutex<mpsc::Receiver<ProviderEvent>>>,
    cached_devs: Arc<Mutex<Vec<Device>>>,
}
impl V4l2Provider {
    pub fn devices(&self) -> Vec<String> {
        self.cached_devs
            .lock()
            .iter()
            .map(|dev| Self::get_id(dev))
            .collect::<Vec<_>>()
    }
}
impl CamProvider for V4l2Provider {
    fn init() -> Self {
        let inner = DeviceProviderFactory::find("v4l2deviceprovider")
            .unwrap()
            .load()
            .unwrap()
            .get()
            .unwrap();

        let (tx, rx) = mpsc::channel(64);
        let rx = Arc::new(Mutex::new(rx));
        let cached_devs = Arc::new(Mutex::new(Vec::new()));

        Self {
            inner,
            rx,
            tx,
            cached_devs,
        }
    }
    fn get_id(dev: &Device) -> String {
        dev.property::<Structure>("properties")
            .get::<String>("device.bus_path")
            .unwrap()
    }
    fn get_by_id(&self, id: String) -> Option<Device> {
        for dev in self.cached_devs.lock().iter() {
            if Self::get_id(&dev) == id {
                return Some(dev.clone());
            }
        }

        None
    }
    fn start(&self) {
        self.register_handler();
        if !self.inner().is_started() {
            self.inner().start().unwrap();
        }
    }
    fn inner(&self) -> &DeviceProvider {
        &self.inner
    }
}
impl V4l2Provider {
    pub(crate) fn register_handler(&self) {
        let cached_devs = self.cached_devs.clone();

        self.inner()
            .bus()
            .set_sync_handler(move |_bus: &Bus, msg: &Message| {
                match msg.view() {
                    MessageView::DeviceAdded(msg) => {
                        let dev = msg.device();

                        cached_devs.lock().push(dev.clone());
                    }
                    MessageView::DeviceRemoved(msg) => {
                        cached_devs
                            .lock()
                            .retain(|dev| Self::get_id(dev) != Self::get_id(&msg.device()));
                    }
                    _ => unimplemented!(),
                }

                BusSyncReply::Pass
            });
    }
}

//impl Freezable for V4l2Provider {}
//
//impl CuSrcTask for V4l2Provider {
//    type Output<'m> = ProviderEvent;
//    type Resources<'r> = ();
//
//    fn new(_config: Option<&cu29::prelude::ComponentConfig>, _resources: Self::Resources<'_>) -> cu29::CuResult<Self>
//    where
//        Self: Sized
//    {
//        Ok(Self::init())
//    }
//
//    fn start(&mut self, _clock: &cu29::prelude::RobotClock) -> cu29::CuResult<()> {
//        self.register_handler(self.tx);
//        Ok(())
//    }
//
//    fn stop(&mut self, _clock: &cu29::prelude::RobotClock) -> cu29::CuResult<()> {
//        self.unregister_handler();
//        Ok(())
//    }
//
//    fn process<'o>(&mut self, clock: &cu29::prelude::RobotClock, new_msg: &mut Self::Output<'o>) -> cu29::CuResult<()> {
//        if let Some(mut event) = self.rx.try_lock() {
//            if let Some(ev) = event.try_recv().ok() {
//                *new_msg = ev;
//            }
//        }
//
//        Ok(())
//    }
//}

//pub struct CamProviderBundle;
//bundle_resources!(CamProviderBundle: V4L2);
//
//impl ResourceBundle for CamProviderBundle {
//    fn build(
//        bundle: cu29::prelude::BundleContext<Self>,
//        _config: Option<&cu29::prelude::ComponentConfig>,
//        manager: &mut cu29::prelude::ResourceManager,
//    ) -> cu29::CuResult<()> {
//        let v4l2_key = bundle.key(CamProviderBundleId::V4L2);
//
//        manager.add_owned(v4l2_key, PROVIDER.clone())?;
//
//        Ok(())
//    }
//}
