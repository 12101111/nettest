use crate::{Measurement, Task};
use anyhow::{anyhow, Context, Result};
use log::info;
use rand::{Rng, SeedableRng};
use rand_xoshiro::Xoshiro256Plus;
use socket2::{Domain, Protocol, Socket, Type};
use std::convert::{TryFrom, TryInto};
use std::io::ErrorKind;
use std::mem::size_of;
use std::net::SocketAddr;
use std::time::{Duration, Instant, SystemTime};

#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(u8)]
enum IcmpType {
    EchoReply = 0,
    EchoRequest = 8,
    EchoRequestV6 = 128,
    EchoReplyV6 = 129,
}

impl TryFrom<u8> for IcmpType {
    type Error = anyhow::Error;
    fn try_from(buf: u8) -> Result<Self> {
        match buf {
            0 => Ok(Self::EchoReply),
            8 => Ok(Self::EchoRequest),
            128 => Ok(Self::EchoRequestV6),
            129 => Ok(Self::EchoReplyV6),
            _ => Err(anyhow!("Not related to ICMP Ping: {:?}", { buf })),
        }
    }
}

#[derive(Copy, Clone, Debug)]
struct Icmphdr {
    icmp_type: IcmpType,
    /// constant: 0
    code: u8,
    /// caculated by kernel
    checksum: u16,
    /// input by kernel
    id: u16,
    seq: u16,
    /// This is not a part of ICMP header
    time_stamp: u64,
}

impl TryFrom<&[u8]> for Icmphdr {
    type Error = anyhow::Error;
    fn try_from(buf: &[u8]) -> Result<Self> {
        if buf.len() < size_of::<Self>() {
            Err(anyhow!(
                "Buffer too short: expect {} bytes, actual: {} bytes",
                size_of::<Self>(),
                buf.len()
            ))
        } else {
            Ok(Icmphdr {
                icmp_type: buf[0].try_into()?,
                code: buf[1],
                checksum: u16::from_be_bytes([buf[2], buf[3]]),
                id: u16::from_be_bytes([buf[4], buf[5]]),
                seq: u16::from_be_bytes([buf[6], buf[7]]),
                time_stamp: u64::from_le_bytes(buf[8..16].try_into()?),
            })
        }
    }
}

impl Icmphdr {
    fn echo(seq: u16, v6: bool) -> Self {
        Icmphdr {
            icmp_type: if v6 {
                IcmpType::EchoRequestV6
            } else {
                IcmpType::EchoRequest
            },
            code: 0,
            checksum: 0,
            id: 0,
            seq,
            time_stamp: SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
    fn bump(&mut self) {
        self.seq = self.seq.overflowing_add(1).0;
        self.time_stamp = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }
    fn vec(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(size_of::<Self>());
        buf.push(self.icmp_type as u8);
        buf.push(self.code);
        buf.extend_from_slice(&self.checksum.to_be_bytes());
        buf.extend_from_slice(&self.id.to_be_bytes());
        buf.extend_from_slice(&self.seq.to_be_bytes());
        buf.extend_from_slice(&self.time_stamp.to_le_bytes());
        buf
    }
}

pub struct PingTask {
    socket: Socket,
    target: SocketAddr,
    echo_hdr: Icmphdr,
    size: usize,
}

impl PingTask {
    pub fn new(addr: &str, size: usize, timeout: u64) -> Result<PingTask> {
        let ip = addr.parse().unwrap_or({
            use std::net::ToSocketAddrs;
            let socket = &format!("{}:0", addr);
            socket
                .to_socket_addrs()
                .context("Can't resolve IP address")?
                .next()
                .ok_or(anyhow!("Don't have IP address"))?
                .ip()
        });
        info!("PING ({}) {} bytes of data.", ip, size);
        let socket = if ip.is_ipv4() {
            Socket::new(Domain::ipv4(), Type::dgram(), Some(Protocol::icmpv4()))
        } else if ip.is_ipv6() {
            Socket::new(Domain::ipv6(), Type::dgram(), Some(Protocol::icmpv6()))
        } else {
            unreachable!("IP is either ipv4 or ipv6")
        };
        let socket = match socket{
            Ok(s) => s,
            Err(e) if e.kind() == ErrorKind::PermissionDenied => {
                return Err(anyhow!("Failed to create ICMP socket: {}, you need to run `sudo sysctl -w net.ipv4.ping_group_range='0 1000'`",e))
            },
            Err(e) => return Err(anyhow!("Failed to create ICMP socket: {}",e)),
        };
        socket.set_read_timeout(Some(Duration::from_secs(timeout)))?;
        Ok(PingTask {
            socket,
            target: SocketAddr::new(ip, 0),
            echo_hdr: Icmphdr::echo(0, ip.is_ipv6()),
            size: size.max(size_of::<Icmphdr>()),
        })
    }
}

impl Task for PingTask {
    fn run(&mut self) -> Result<Measurement> {
        let rand_str = Xoshiro256Plus::seed_from_u64(self.echo_hdr.time_stamp)
            .sample_iter(&rand::distributions::Alphanumeric)
            .map(|x| x as u8)
            .take(self.size - size_of::<Icmphdr>());
        self.echo_hdr.bump();
        let mut buffer = self.echo_hdr.vec();
        buffer.extend(rand_str);
        let send_time = Instant::now();
        let size = self
            .socket
            .send_to(&buffer, &self.target.into())
            .context("Failed to send echo request")?;
        if size != buffer.len() {
            return Err(anyhow!(
                "Failed to send echo request({} bytes), only {} bytes sent",
                buffer.len(),
                size
            ));
        }
        let (size, addr) = self
            .socket
            .recv_from(&mut buffer)
            .map_err(|e| match e.kind() {
                ErrorKind::WouldBlock | ErrorKind::TimedOut => anyhow!("Timed out"),
                _ => anyhow!("Failed to receive echo reply: {:?}", e),
            })?;
        let time = send_time.elapsed();
        let recv_hdr: Icmphdr = buffer.as_slice().try_into().context("Packet is broken")?;
        let ip = addr.as_std().unwrap().ip();
        if ip.is_ipv4() && recv_hdr.icmp_type != IcmpType::EchoReply
            || ip.is_ipv6() && recv_hdr.icmp_type != IcmpType::EchoReplyV6
        {
            return Err(anyhow!("Received packet isn't echo reply"));
        }
        if ip != self.target.ip() {
            return Err(anyhow!("Received packet isn't sent from target"));
        }
        info!(
            "{} bytes from {}: icmp_seq={} time={:?}",
            size, ip, recv_hdr.seq, time
        );
        Ok(Measurement::Time(time))
    }
}

#[test]
fn ping_localhost() {
    let mut ping = PingTask::new("127.0.0.1".into(), 64, 1).unwrap();
    let _ = ping.run().unwrap();
}

#[test]
fn dont_resolve() {
    assert!(PingTask::new("thissitedontexist.abc".into(), 64, 1).is_err())
}
