use std::{net::Ipv4Addr, str::FromStr, sync::LazyLock};

use nt_client::{NTAddr, NewClientOptions};

use crate::config::{Cfg, Config};

#[allow(non_upper_case_globals)]
pub static Nt: LazyLock<nt_client::Client> = LazyLock::new(|| {
    // TODO: How bad is this for perf? Would futures_executor be better?
    futures_executor::block_on(async {
        // Come up with an IP address for the roboRIO based on the team number or specified IP
        let Config {
            ntables_ip,
            team_number,
            ..
        } = &*Cfg.read();
        let addr = if let Some(ntables_ip) = ntables_ip {
            NTAddr::Custom(Ipv4Addr::from_str(ntables_ip).unwrap())
        } else {
            NTAddr::TeamNumber(*team_number)
        };

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
            addr,
            name: dev_name,
            ..Default::default()
        });

        //info!("Connected to NT server at {roborio_ip:?} successfully!");

        nt
    })
});
