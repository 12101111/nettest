use super::MB;
use crate::{Measurement, Task};
use anyhow::{anyhow, Context, Result};
use log::info;
use rand::{Rng, SeedableRng};
use rand_xoshiro::Xoshiro256Plus;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant, SystemTime};

pub struct SpeedtestUploadTask {
    addr: SocketAddr,
    timeout: Duration,
    size: usize,
}

impl SpeedtestUploadTask {
    pub fn new(
        server: &str,
        sponsor: &str,
        size: usize,
        timeout: u64,
    ) -> Result<SpeedtestUploadTask> {
        info!(
            "Speedtest upload test, connecting to {} ({})",
            sponsor, server
        );
        Ok(SpeedtestUploadTask {
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

impl Task for SpeedtestUploadTask {
    fn run(&mut self) -> Result<Measurement> {
        let mut stream = TcpStream::connect_timeout(&self.addr, self.timeout)?;
        stream.set_read_timeout(Some(self.timeout))?;
        stream.set_write_timeout(Some(self.timeout))?;
        info!("Upload {} MiB to {}", self.size, self.addr);
        let mut buffer = Vec::with_capacity(2 * MB + 2);
        let time_stamp = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let mut rng = Xoshiro256Plus::seed_from_u64(time_stamp);
        for _ in 0..2 * MB {
            buffer.push(rng.gen_range(0x20, 0x7F));
        }
        buffer.extend(b"\r\n");
        let size = self.size * MB;
        let ulstring = format!("UPLOAD {} 0\r\n", size);
        stream.write_all(ulstring.as_bytes())?;
        let rand_size = size - ulstring.len();
        let mut line = String::new();
        let now = Instant::now();
        let mut old = now;
        let mut len = 0;
        let mut old_len = 0;
        let step = (size / 32).max(MB);
        loop {
            let (start, end) = if len + MB < rand_size {
                let start = rng.gen_range(0, MB);
                (start, start + MB)
            } else {
                let left = 2 * MB - (rand_size - len);
                (left, buffer.len())
            };
            stream.write_all(&buffer[start..end])?;
            len += end - start;
            let len_since_last_measure = len - old_len;
            if len_since_last_measure >= step {
                let time = old.elapsed().as_micros();
                info!(
                    "Size: {:.3} MiB, time: {:?} ms, speed: {:.3} Mbps",
                    len_since_last_measure as f64 / (MB as f64),
                    time as f64 / 1000.0,
                    len_since_last_measure as f64 / (time as f64) * 8.0
                );
                old = Instant::now();
                old_len = len;
            }
            if buffer.len() == end {
                break;
            }
        }
        let mut reader = BufReader::new(stream);
        reader.read_line(&mut line)?;
        let time = now.elapsed();
        Ok(Measurement::Speed(len, time))
    }
}
