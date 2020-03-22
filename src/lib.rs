mod ping;
pub mod speedtest;
mod tcping;
mod udping;

pub use ping::PingTask;
pub use tcping::TcpingTask;
pub use udping::UdpingTask;

pub trait Task {
    fn run(&mut self) -> anyhow::Result<Measurement>;
}

#[derive(Debug)]
pub enum Measurement {
    Time(std::time::Duration),
    Speed(usize, std::time::Duration),
}