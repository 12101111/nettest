use super::MB;
use crate::{Measurement, Task};
use anyhow::{anyhow, Context, Result};
use log::info;
use rand::{Rng, SeedableRng};
use rand_xoshiro::Xoshiro256Plus;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

pub struct TcpuploadTask {
    addr: SocketAddr,
    timeout: Duration,
    size: usize,
}

impl TcpuploadTask {
    pub fn new(server: &str, size: usize, timeout: u64) -> Result<TcpuploadTask> {
        info!("TCP upload test, connecting to {}", server);
        Ok(TcpuploadTask {
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

impl Task for TcpuploadTask {
    fn run(&mut self) -> Result<Measurement> {
        let mut stream = TcpStream::connect_timeout(&self.addr, self.timeout)?;
        stream.set_read_timeout(Some(self.timeout))?;
        stream.set_write_timeout(Some(self.timeout))?;
        info!("Upload {} MiB to {}", self.size, self.addr);
        let mut rand_pool = Vec::with_capacity(2 * MB);
        let mut rng = Xoshiro256Plus::from_entropy();
        for _ in 0..2 * MB {
            rand_pool.push(rng.gen_range(0x20, 0x7F));
        }
        let size = self.size * MB;
        let ulstring = format!("UPLOAD {} 0\r\n", size);
        stream.write_all(ulstring.as_bytes())?;
        let mut rand_size = size - ulstring.len() - 1; // \r\n count into 1 byte, wierd
        let mut line = String::new();
        let now = Instant::now();
        let mut old = now;
        let mut old_size = rand_size;
        let step = (size / 32).max(MB);
        while rand_size > 0 {
            let start = rng.gen_range(0, MB);
            let len = rand_size.min(MB);
            stream.write_all(&rand_pool[start..start + len])?;
            rand_size -= len;
            let len_since_last_measure = old_size - rand_size;
            if len_since_last_measure >= step {
                let time = old.elapsed().as_micros();
                info!(
                    "Size: {:.3} MiB, time: {:?} ms, speed: {:.3} Mbps",
                    len_since_last_measure as f64 / (MB as f64),
                    time as f64 / 1000.0,
                    len_since_last_measure as f64 / (time as f64) * 8.0
                );
                old = Instant::now();
                old_size = rand_size;
            }
        }
        stream.write_all(b"\r\n")?;
        let mut reader = BufReader::new(stream);
        reader.read_line(&mut line)?;
        let time = now.elapsed();
        stream = reader.into_inner();
        let _ = stream.write_all(b"QUIT\r\n");
        Ok(Measurement::Speed(size, time))
    }
}
