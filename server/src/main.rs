//mod quic;
mod tcp;
use std::net;

const MB: usize = 1024 * 1024;

#[async_std::main]
async fn main() {
    let mut args = std::env::args();
    let cmd = args.next().unwrap();
    if args.len() != 1 {
        println!("Usage: `{} [PORT]", cmd);
        return;
    }
    let port: u16 = args.next().unwrap().parse().unwrap();
    let socker = net::SocketAddr::new(net::Ipv4Addr::UNSPECIFIED.into(), port);
    //let quic = tokio::spawn(async { quic::server(port).await });

    if let Err(e) = tcp::server(socker).await {
        eprintln!("Err: {}", e);
    }
}
