use std::sync::LazyLock;

use nt_client::{NTAddr, NewClientOptions};

use crate::config::{Cfg, Config};

#[allow(non_upper_case_globals)]
pub static Nt: LazyLock<nt_client::Client> = LazyLock::new(|| {
    tokio::runtime::LocalRuntime::new()
        .unwrap()
        .block_on(async {
            // Come up with an IP address for the roboRIO based on the team number or specified IP
            let Config {
                ntables_ip: _,
                team_number,
                ..
            } = &*Cfg.read();

            // Get the device's name or generate one if not set
            let dev_name = if let Some(dev_name) = (*Cfg.read()).device_name.clone() {
                dev_name
            } else {
                warn!("device name not set! generating one...");

                // Generate & save it
                let dev_name = String::from("chalkydri");
                (*Cfg.write()).device_name = Some(dev_name.clone());

                dev_name
            };

            let nt = nt_client::Client::new(NewClientOptions {
                addr: NTAddr::TeamNumber(*team_number),
                name: dev_name,
                ..Default::default()
            });

            //info!("Connected to NT server at {roborio_ip:?} successfully!");

            nt
        })
});
