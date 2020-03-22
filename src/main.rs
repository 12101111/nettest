use anyhow::{anyhow, Result};
use log::*;
use nettest::*;
use std::time::Duration;
use structopt::StructOpt;

static LOGGER: Logger = Logger;

struct Logger;

impl Log for Logger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }
    fn log(&self, record: &Record) {
        if record.level() == log::Level::Info {
            println!("{}", record.args());
        } else {
            println!("[{:<5}] {}", record.level(), record.args());
        }
    }
    fn flush(&self) {}
}

#[derive(StructOpt)]
#[structopt(about = "All-in-one network test tool")]
struct Opt {
    #[structopt(subcommand)]
    cmd: Command,
    /// Count of times to test
    #[structopt(global = true, short, long)]
    count: Option<usize>,
    /// Wait interval ms between echo test
    #[structopt(global = true, short, long, default_value = "1000")]
    interval: u64,
    /// Timeout of each test (in seconds)
    #[structopt(global = true, short, long, default_value = "5")]
    timeout: u64,
}

#[derive(Debug, StructOpt)]
pub enum Command {
    /// Measuring latency using ICMP or ICMPv6 echo"
    Ping {
        /// IP or hostname of target
        address: String,
        /// Number of bytes to test
        #[structopt(short, long, default_value = "56")]
        size: usize,
    },
    /// Measuring latency of TCP shake hands
    Tcping {
        /// IP or hostname and port of target.
        /// example: `127.0.0.1:8080` of `github.com:443`
        address: String,
    },
    /// Measuring latency using UDP echo. use `socat -v UDP-LISTEN:8000,fork PIPE` to start a server"
    Udping {
        /// IP or hostname of target
        address: String,
        /// Number of bytes to test
        #[structopt(short, long, default_value = "56")]
        size: usize,
    },
    /// Measuring latency and bandwidth using Speedtest.
    Speedtest {
        #[structopt(subcommand)]
        cmd: SpeedtestCmd,
    },
}

#[derive(Debug, StructOpt)]
pub enum SpeedtestCmd {
    /// Lists available servers
    List,
    /// Ping test
    Ping {
        /// Specify id of server to test, id can get from `list` subcommand
        #[structopt(short = "I", long)]
        id: Option<String>,
        /// Specify hostname of server to test
        #[structopt(short = "n", long)]
        host: Option<String>,
    },
    /// Upload test
    Upload {
        /// Number of MegaBytes to upload
        #[structopt(short, long, default_value = "20")]
        size: usize,
        /// Specify id of server to test, id can get from `list` subcommand
        #[structopt(short = "I", long)]
        id: Option<String>,
        /// Specify hostname of server to test
        #[structopt(short = "n", long)]
        host: Option<String>,
    },
    /// Download test
    Download {
        /// Number of MegaBytes to download
        #[structopt(short, long, default_value = "100")]
        size: usize,
        /// Specify id of server to test, id can get from `list` subcommand
        #[structopt(short = "I", long)]
        id: Option<String>,
        /// Specify hostname of server to test
        #[structopt(short = "n", long)]
        host: Option<String>,
    },
}

fn main() {
    let opt = Opt::from_args();
    log::set_logger(&LOGGER).expect("Set logger failed");
    log::set_max_level(LevelFilter::Info);
    if let Err(e) = run(opt) {
        error!("{}", e);
        std::process::exit(1);
    }
}

fn run(opt: Opt) -> Result<()> {
    if opt.count == Some(0) {
        return Err(anyhow!("count = 0 means don't run anything"));
    }
    use Command::*;
    let mut task: Box<dyn Task> = match opt.cmd {
        Ping { address, size } => Box::new(PingTask::new(&address, size, opt.timeout)?),
        Tcping { address } => Box::new(TcpingTask::new(&address, opt.timeout)?),
        Udping { address, size } => Box::new(UdpingTask::new(&address, size, opt.timeout)?),
        Speedtest { mut cmd } => {
            if let SpeedtestCmd::List = cmd {
                for s in speedtest::list_servers()? {
                    println!("{}", s);
                }
                return Ok(());
            }
            // workaround for https://github.com/clap-rs/clap/issues/1570
            let (id, host) = match &mut cmd {
                SpeedtestCmd::Ping { id, host, .. } => (id.take(), host.take()),
                SpeedtestCmd::Upload { id, host, .. } => (id.take(), host.take()),
                SpeedtestCmd::Download { id, host, .. } => (id.take(), host.take()),
                _ => unreachable!(),
            };
            let mut servers = if let Some(h) = host {
                vec![speedtest::Server::custom(h)]
            } else {
                speedtest::list_servers()?
            };
            let server_from_id = id.and_then(|id| servers.iter().find(|s| s.id == id));
            let server = match server_from_id {
                Some(server) => server,
                None => match servers.len() {
                    0 => return Err(anyhow!("Can't find any server avaliable")),
                    1 => &servers[0],
                    _ => speedtest::best_server(&mut servers, opt.timeout)?,
                },
            };
            match cmd {
                SpeedtestCmd::Ping { .. } => Box::new(speedtest::SpeedtestPingTask::new(
                    &server.host,
                    &server.sponsor,
                    true,
                    opt.timeout,
                )?),
                SpeedtestCmd::Upload { size, .. } => Box::new(speedtest::SpeedtestUploadTask::new(
                    &server.host,
                    &server.sponsor,
                    size,
                    opt.timeout,
                )?),
                SpeedtestCmd::Download { size, .. } => {
                    Box::new(speedtest::SpeedtestDownloadTask::new(
                        &server.host,
                        &server.sponsor,
                        size,
                        opt.timeout,
                    )?)
                }
                _ => unreachable!(),
            }
        }
    };
    let count = opt.count.unwrap_or(5);
    let mut err_count = 0;
    let mut results = Vec::with_capacity(count);
    for _ in 0..count {
        match task.run() {
            Err(e) => {
                info!("{}", e);
                err_count += 1;
            }
            Ok(r) => results.push(r),
        }
        std::thread::sleep(Duration::from_millis(opt.interval));
    }
    analisys(results, err_count);
    Ok(())
}

fn analisys(results: Vec<Measurement>, err_count: usize) {
    if results.len() == 0 {
        error!("All tests are failed");
    }
    info!("--- statistics ---");
    match results[0] {
        Measurement::Speed(_l, _t) => {
            let speeds: Vec<(usize, Duration)> = results
                .iter()
                .map(|s| {
                    if let Measurement::Speed(l, t) = s {
                        (*l, *t)
                    } else {
                        unreachable!()
                    }
                })
                .collect();
            let min = speeds
                .iter()
                .min_by(|x, y| (x.0 as u32 * y.1).cmp(&(y.0 as u32 * x.1)))
                .unwrap();
            let max = speeds
                .iter()
                .max_by(|x, y| (x.0 as u32 * y.1).cmp(&(y.0 as u32 * x.1)))
                .unwrap();
            let lens: usize = speeds.iter().map(|x| x.0).sum();
            let times: Duration = speeds.iter().map(|x| x.1).sum();
            let speed = |x: &(usize, Duration)| (x.0 * 8) as f64 / (x.1.as_micros() as f64);
            info!("{} MiB transmitted in {:?}", lens / (1024 * 1024), times);
            info!(
                "Speed min/avg/max {:.3}/{:.3}/{:.3}Mbps",
                speed(min),
                speed(&(lens, times)),
                speed(max)
            );
        }
        Measurement::Time(_t) => {
            let times: Vec<Duration> = results
                .iter()
                .map(|t| {
                    if let Measurement::Time(t) = t {
                        *t
                    } else {
                        unreachable!()
                    }
                })
                .collect();
            let count = times.len() + err_count;
            let received = times.len();
            let total: Duration = times.iter().sum();
            let min = *times.iter().min().unwrap();
            let max = *times.iter().max().unwrap();
            let avg = total / (received as u32);
            let ms = |t| format!("{:.3}", t.as_micros() as f64 / 1000.0);
            let mdev = ((times
                .iter()
                .map(|t| t.as_nanos() as i128)
                .map(|t| t - (avg.as_nanos() as i128))
                .map(|t| t * t)
                .sum::<i128>()) as f64
                / received as f64)
                .sqrt()
                / 1000_000.0;
            info!(
                "{} packets transmitted, {} received, {:.2}% packet loss, time {} ms",
                count,
                received,
                100.0 * err_count as f64 / (count as f64),
                ms(total)
            );
            info!(
                "rtt min/avg/max/mdev = {}/{}/{}/{:.3} ms",
                ms(min),
                ms(avg),
                ms(max),
                mdev
            );
        }
    }
}
