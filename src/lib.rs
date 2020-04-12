mod ping;
mod quicdownload;
mod tcpdownload;
mod tcping;
mod tcpupload;
mod udping;

pub use ping::PingTask;
pub use quicdownload::QuicdownloadTask;
pub use tcpdownload::TcpdownloadTask;
pub use tcping::TcpingTask;
pub use tcpupload::TcpuploadTask;
pub use udping::UdpingTask;

const MB: usize = 1024 * 1024;

pub trait Task {
    fn run(&mut self) -> anyhow::Result<Measurement>;
}

#[derive(Debug)]
pub enum Measurement {
    Time(std::time::Duration),
    Speed(usize, std::time::Duration),
}
