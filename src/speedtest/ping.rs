use crate::{Measurement, Task};
use anyhow::{anyhow, Context, Result};
use log::info;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

pub struct SpeedtestPingTask {
    sponsor: String,
    addr: SocketAddr,
    timeout: Duration,
    seq: u16,
    log: bool,
}

impl SpeedtestPingTask {
    pub fn new(server: &str, sponsor: &str, log: bool, timeout: u64) -> Result<SpeedtestPingTask> {
        if log {
            info!(
                "Speedtest ping test, connecting to {} ({})",
                sponsor, server
            );
        }
        Ok(SpeedtestPingTask {
            sponsor: sponsor.to_owned(),
            addr: server
                .to_socket_addrs()
                .context("Can't resolve IP address")?
                .next()
                .ok_or(anyhow!("Don't have IP address"))?,
            timeout: Duration::from_secs(timeout),
            seq: 0,
            log,
        })
    }
}

impl Task for SpeedtestPingTask {
    fn run(&mut self) -> Result<Measurement> {
        self.seq += 1;
        let mut line = String::with_capacity(48);
        let mut stream = TcpStream::connect_timeout(&self.addr, self.timeout)?;
        stream.set_read_timeout(Some(self.timeout))?;
        stream.set_write_timeout(Some(self.timeout))?;
        let now = Instant::now();
        stream.write_all(b"HI\r\n")?;
        let mut reader = BufReader::new(stream);
        reader.read_line(&mut line)?;
        let time = now.elapsed();
        if self.log {
            info!(
                "{} bytes from {}: seq={} time={:?}",
                line.len(),
                self.sponsor,
                self.seq,
                time
            );
        }
        Ok(Measurement::Time(time))
    }
}
