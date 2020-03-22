use crate::{Measurement, Task};
use anyhow::{anyhow, Context, Result};
use log::info;
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

pub struct TcpingTask {
    target: SocketAddr,
    timeout: Duration,
    seq: u16,
}

impl TcpingTask {
    pub fn new(addr: &str, timeout: u64) -> Result<TcpingTask> {
        let target = addr
            .to_socket_addrs()
            .context("Can't resolve IP address")?
            .next()
            .ok_or(anyhow!("Don't have IP address"))?;
        let format_target = target.to_string();
        if format_target != addr {
            info!("Ping to {} ({}) using TCP", addr, format_target);
        } else {
            info!("Ping to {} using TCP", format_target);
        }
        let timeout = Duration::from_secs(timeout);
        Ok(Self {
            target,
            timeout,
            seq: 0,
        })
    }
}

impl Task for TcpingTask {
    fn run(&mut self) -> Result<Measurement> {
        self.seq += 1;
        let start = Instant::now();
        let tcp = TcpStream::connect_timeout(&self.target, self.timeout)?;
        let time = start.elapsed();
        tcp.shutdown(std::net::Shutdown::Both)?;
        drop(tcp);
        info!(
            "Connected to {}: seq={} time={:?}",
            self.target, self.seq, time
        );
        Ok(Measurement::Time(time))
    }
}
