use tokio::sync::mpsc;

use gstreamer::{
    Bus, BusSyncReply, Device, DeviceProvider, DeviceProviderFactory, Message, MessageView,
    Structure, prelude::*,
};

/// An event from a camera provider
pub(crate) enum ProviderEvent {
    Connected(String, Device),
    Disconnected(String, Device),
}

pub trait CamProvider {
    fn init() -> Self
    where
        Self: Sized;
    /// Get a unique ID for the given device
    fn get_id(dev: &Device) -> String;
    fn inner(&self) -> &DeviceProvider;

    fn register_handler(&self, tx: mpsc::Sender<ProviderEvent>) {
        self.inner()
            .bus()
            .set_sync_handler(move |_bus: &Bus, msg: &Message| {
                match msg.view() {
                    MessageView::DeviceAdded(msg) => {
                        let dev = msg.device();
                        let id = Self::get_id(&dev);

                        tx.blocking_send(ProviderEvent::Connected(id, dev)).unwrap();
                    }
                    MessageView::DeviceRemoved(msg) => {
                        let dev = msg.device();
                        let id = Self::get_id(&dev);

                        tx.blocking_send(ProviderEvent::Disconnected(id, dev))
                            .unwrap();
                    }
                    _ => unimplemented!(),
                }

                BusSyncReply::Pass
            });
    }

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

pub struct V4l2Provider {
    inner: DeviceProvider,
}
impl CamProvider for V4l2Provider {
    fn init() -> Self {
        let inner = DeviceProviderFactory::find("v4l2deviceprovider")
            .unwrap()
            .load()
            .unwrap()
            .get()
            .unwrap();

        Self { inner }
    }
    fn get_id(dev: &Device) -> String {
        dev.property::<Structure>("properties")
            .get::<String>("device.serial")
            .unwrap()
    }
    fn inner(&self) -> &DeviceProvider {
        &self.inner
    }
}
