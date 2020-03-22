use crate::{Measurement, Task};
use anyhow::Result;
use log::info;
use serde::Deserialize;
use std::fmt;
use std::time::Duration;

mod download;
mod ping;
mod upload;

pub use download::SpeedtestDownloadTask;
pub use ping::SpeedtestPingTask;
pub use upload::SpeedtestUploadTask;

const MB: usize = 1024 * 1024;

#[derive(Clone, Deserialize, Default)]
pub struct Server {
    pub lat: String,
    pub lon: String,
    pub distance: i32,
    pub name: String,
    pub country: String,
    pub cc: String,
    pub sponsor: String,
    pub id: String,
    pub host: String,
    #[serde(skip)]
    pub latency: Duration,
}

impl Server {
    pub fn custom(host: String) -> Server {
        Server {
            host,
            ..Server::default()
        }
    }
}

impl fmt::Display for Server {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "[id: {:5}] {:4}Km [{}, {}] {}\n   {}",
            self.id, self.distance, self.name, self.cc, self.sponsor, self.host
        )
    }
}

pub fn list_servers() -> Result<Vec<Server>> {
    info!("Fetch server list from speedtest.net ...");
    let res = ureq::get("https://www.speedtest.net/api/js/servers?engine=js")
        .timeout_connect(10_000)
        .timeout_read(10_000)
        .call();
    Ok(serde_json::from_value(res.into_json()?)?)
}

pub fn best_server(servers: &mut Vec<Server>, timeout: u64) -> Result<&Server> {
    info!("Finding best server...");
    servers.sort_by_key(|s| s.distance);
    servers.truncate(5);
    servers.iter_mut().for_each(|s| {
        let ping = ping::SpeedtestPingTask::new(&s.host, &s.sponsor, false, timeout);
        let timeout = Duration::from_secs(timeout);
        s.latency = match ping {
            Err(_) => timeout,
            Ok(mut task) => {
                if let Ok(Measurement::Time(t)) = task.run() {
                    t
                } else {
                    timeout
                }
            }
        };
        info!("[{:5}] {}: {:?}", s.id, s.sponsor, s.latency);
    });
    servers.sort_by(|a, b| a.latency.partial_cmp(&b.latency).unwrap());
    let best = &servers[0];
    info!("Select server {}", best.sponsor);
    Ok(best)
}
