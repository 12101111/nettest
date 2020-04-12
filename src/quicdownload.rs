use super::MB;
use crate::{Measurement, Task};
use anyhow::{anyhow, Context, Result};
use log::info;
use rand::{Rng, SeedableRng};
use rand_xoshiro::Xoshiro256Plus;
use std::net::{self, SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::{Duration, Instant};

const MAX_DATAGRAM_SIZE: usize = 1350;
const DOWNLOAD_ID: u64 = 3;

pub struct QuicdownloadTask {
    addr: SocketAddr,
    socket: UdpSocket,
    conn: Box<quiche::Connection>,
    size: usize,
}

impl QuicdownloadTask {
    pub fn new(addr: &str, size: usize, timeout: u64) -> Result<QuicdownloadTask> {
        let peer_addr = addr
            .to_socket_addrs()
            .context("Can't resolve IP address")?
            .next()
            .ok_or(anyhow!("Don't have IP address"))?;
        let localaddr = match peer_addr {
            SocketAddr::V4(_) => net::Ipv4Addr::UNSPECIFIED.into(),
            SocketAddr::V6(_) => net::Ipv6Addr::UNSPECIFIED.into(),
        };
        let socket = UdpSocket::bind(SocketAddr::new(localaddr, 0))?;
        socket
            .connect(peer_addr)
            .with_context(|| anyhow!("Failed to connect to {}", peer_addr))?;
        let timeout = Duration::from_secs(timeout);
        socket.set_read_timeout(Some(timeout))?;
        socket.set_write_timeout(Some(timeout))?;
        let format_addr = peer_addr.to_string();
        if format_addr != addr {
            info!(
                "QUIC download test, connecting to {} ({})",
                addr, format_addr
            );
        } else {
            info!("QUIC download test, connecting to {}", addr);
        }
        let mut config = quiche::Config::new(quiche::PROTOCOL_VERSION).unwrap();
        config.verify_peer(false);
        config.set_application_protos(b"\x13speedtest/0.1")?;
        config.set_disable_active_migration(true);
        config.set_max_idle_timeout(timeout.as_millis() as u64);
        let mut scid = vec![0u8; quiche::MAX_CONN_ID_LEN];
        Xoshiro256Plus::from_entropy().fill(scid.as_mut_slice());
        let mut conn = std::pin::Pin::into_inner(quiche::connect(None, &scid, &mut config)?);
        let mut out = vec![0; MAX_DATAGRAM_SIZE];
        loop {
            let write = match conn.send(&mut out) {
                Ok(v) => v,
                Err(quiche::Error::Done) => break,
                Err(e) => Err(e).context("QUIC handshake failed")?,
            };
            socket
                .send(&out[..write])
                .context("Failed to establish QUIC handshake")?;
        }
        let mut init_buf = [0u8; 1024];
        loop {
            let len = socket.recv(&mut init_buf)?;
            // Process potentially coalesced packets.
            match conn.recv(&mut init_buf[..len]) {
                Ok(v) => v,
                Err(quiche::Error::Done) => break,
                Err(e) => Err(e).context("Fail to receive from peer")?,
            };
        }
        if conn.is_closed() {
            return Err(anyhow!("connection closed, {:?}", conn.stats()));
        }
        Ok(Self {
            addr: peer_addr,
            conn,
            socket,
            size,
        })
    }
}

impl Task for QuicdownloadTask {
    fn run(&mut self) -> Result<Measurement> {
        assert!(self.conn.is_established());
        info!("Download {} MiB from {}", self.size, self.addr);
        let dlstring = format!("DOWNLOAD {}\r\n", self.size * MB);
        self.conn
            .stream_send(DOWNLOAD_ID, dlstring.as_bytes(), true)
            .unwrap();
        const MIN_PACKET_SIZE: usize = MB;
        let step = (self.size * MB / 32).max(MIN_PACKET_SIZE);
        let mut buf = vec![0u8; step];
        let mut out = vec![0; MAX_DATAGRAM_SIZE];
        let now = Instant::now();
        let mut old = now;
        let mut len = 0;
        let mut old_len = 0;
        for s in self.conn.readable() {
            while let Ok((length, fin)) = self.conn.stream_recv(s, &mut buf) {
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
                if s == DOWNLOAD_ID && fin {
                    self.conn.close(true, 0x00, b"")?;
                }
            }
        }
        loop {
            let write = match self.conn.send(&mut out) {
                Ok(v) => v,
                Err(quiche::Error::Done) => break,
                Err(e) => Err(e).context("Failed to close connection")?,
            };
            self.socket.send(&out[..write])?;
        }
        if self.conn.is_closed() {
            let time = now.elapsed();
            Ok(Measurement::Speed(len, time))
        } else {
            Err(anyhow!("Connection is not closed"))
        }
    }
}
