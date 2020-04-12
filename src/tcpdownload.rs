use super::MB;
use crate::{Measurement, Task};
use anyhow::{anyhow, Context, Result};
use log::info;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

pub struct TcpdownloadTask {
    addr: SocketAddr,
    timeout: Duration,
    size: usize,
}

impl TcpdownloadTask {
    pub fn new(server: &str, size: usize, timeout: u64) -> Result<TcpdownloadTask> {
        info!("TCP download test, connecting to {}", server);
        Ok(TcpdownloadTask {
            addr: server
                .to_socket_addrs()
                .context("Can't resolve IP address")?
                .next()
                .ok_or(anyhow!("Don't have IP address"))?,
            timeout: Duration::from_secs(timeout),
            size,
        })
    }
}

impl Task for TcpdownloadTask {
    fn run(&mut self) -> Result<Measurement> {
        let mut stream = TcpStream::connect_timeout(&self.addr, self.timeout)?;
        stream.set_read_timeout(Some(self.timeout))?;
        stream.set_write_timeout(Some(self.timeout))?;
        info!("Download {} MiB from {}", self.size, self.addr);
        let dlstring = format!("DOWNLOAD {}\r\n", self.size * MB);
        stream.write_all(dlstring.as_bytes())?;
        const MIN_PACKET_SIZE: usize = MB;
        let step = (self.size * MB / 32).max(MIN_PACKET_SIZE);
        let mut reader = BufReader::with_capacity(step / 2, stream);
        let now = Instant::now();
        let mut old = now;
        let mut len = 0;
        let mut old_len = 0;
        loop {
            let buffer = reader.fill_buf()?;
            let length = buffer.len();
            len += length;
            let len_since_last_measure = len - old_len;
            if len_since_last_measure >= step {
                let time = old.elapsed().as_micros();
                info!(
                    "Size: {:.3} MiB, time: {} ms, speed: {:.3} Mbps",
                    len_since_last_measure as f64 / (MB as f64),
                    time as f64 / 1000.0,
                    len_since_last_measure as f64 / (time as f64) * 8.0
                );
                old = Instant::now();
                old_len = len;
            }
            if length == 0 || buffer.last() == Some(&b'\n') {
                break;
            }
            reader.consume(length);
        }
        let time = now.elapsed();
        stream = reader.into_inner();
        let _ = stream.write_all(b"QUIT\r\n");
        Ok(Measurement::Speed(len, time))
    }
}
