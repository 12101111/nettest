use crate::{Measurement, Task};
use anyhow::{anyhow, Context, Result};
use log::info;
use rand::{Rng, SeedableRng};
use rand_xoshiro::Xoshiro256Plus;
use std::io::ErrorKind;
use std::net::{self, SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::{Duration, Instant, SystemTime};

pub struct UdpingTask {
    target: SocketAddr,
    socket: UdpSocket,
    seq: u16,
    size: usize,
}

impl UdpingTask {
    pub fn new(addr: &str, size: usize, timeout: u64) -> Result<UdpingTask> {
        let target = addr
            .to_socket_addrs()
            .context("Can't resolve IP address")?
            .next()
            .ok_or(anyhow!("Don't have IP address"))?;
        let localaddr = match target {
            SocketAddr::V4(_) => net::Ipv4Addr::UNSPECIFIED.into(),
            SocketAddr::V6(_) => net::Ipv6Addr::UNSPECIFIED.into(),
        };
        let socket = UdpSocket::bind(SocketAddr::new(localaddr, 0))?;
        socket
            .connect(target)
            .with_context(|| anyhow!("Failed to connect to {}", target))?;
        let timeout = Duration::from_secs(timeout);
        socket.set_read_timeout(Some(timeout))?;
        socket.set_write_timeout(Some(timeout))?;
        let format_target = target.to_string();
        if format_target != addr {
            info!("Ping to {} ({}) using UDP", addr, format_target);
        } else {
            info!("Ping to {} using UDP", format_target);
        }
        return Ok(Self {
            target,
            socket,
            seq: 0,
            size,
        });
    }
}

impl Task for UdpingTask {
    fn run(&mut self) -> Result<Measurement> {
        self.seq += 1;
        let mut buffer = Vec::with_capacity(self.size);
        let time_stamp = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let rand_str = Xoshiro256Plus::seed_from_u64(time_stamp)
            .sample_iter(&rand::distributions::Alphanumeric)
            .map(|x| x as u8)
            .take(self.size.max(8) - 8);
        buffer.extend(&time_stamp.to_le_bytes());
        buffer.extend(rand_str);
        assert_eq!(buffer.len(), self.size);
        let start = Instant::now();
        let size_send = self.socket.send(&buffer)?;
        let size_recv = self.socket.recv(&mut buffer).map_err(|e| match e.kind() {
            ErrorKind::WouldBlock | ErrorKind::TimedOut => anyhow!("Timed out"),
            _ => anyhow!("Failed to receive echo reply: {}", e),
        })?;
        let time = start.elapsed();
        if self.size != size_send {
            return Err(anyhow!(
                "Expect to send {} bytes but only sent {} bytes",
                self.size,
                size_send,
            ));
        }
        if size_recv != size_send {
            return Err(anyhow!(
                "Send {} bytes but receive {} bytes",
                size_send,
                size_recv
            ));
        }
        info!(
            "{} bytes from {}: seq={} time={:?}",
            size_send, self.target, self.seq, time
        );
        Ok(Measurement::Time(time))
    }
}
