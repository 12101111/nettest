use std::io::{BufRead, BufReader, Read, Write};
use std::net::{self, SocketAddr, TcpListener, TcpStream};
fn main() {
    let mut args = std::env::args();
    let cmd = args.next().unwrap();
    if args.len() != 1 {
        println!("Usage: `{} [PORT]", cmd);
        return;
    }
    let port: u16 = args.next().unwrap().parse().unwrap();
    let socker = SocketAddr::new(net::Ipv4Addr::UNSPECIFIED.into(), port);
    let listener = TcpListener::bind(socker).unwrap();
    let mut incoming = listener.incoming();
    while let Some(stream) = incoming.next() {
        std::thread::spawn(move || handle_stream(stream.unwrap()));
    }
}

const MB: usize = 1024 * 1024;

fn handle_stream(mut stream: TcpStream) {
    let mut reader = BufReader::with_capacity(MB, stream);
    let mut buf = String::new();
    reader.read_line(&mut buf).unwrap();
    stream = reader.into_inner();
    let mut length = buf.len();
    let upload_bytes = dbg!(buf)
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse::<usize>()
        .unwrap();
    let mut buffer = vec![0u8; upload_bytes.min(MB)];
    loop {
        let n = stream.read(&mut buffer).unwrap();
        if n == 0 {
            println!("\x1B[31mReady in EOF\x1B[39m");
            break;
        } else {
            length += n;
            if buffer[n - 1] == b'\n' {
                println!("Ready in \\n");
                break;
            }
        }
    }
    let time_stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let ans = format!("Ok {} {}\n", length - 1, time_stamp);
    stream.write_all(dbg!(ans).as_bytes()).unwrap();
    if upload_bytes > length {
        eprintln!(
            "\x1B[31mDiff in req and res: {}\x1B[39m",
            upload_bytes - length
        );
    }
    println!("Disconnect with {}", stream.peer_addr().unwrap());
    let _ = stream.shutdown(std::net::Shutdown::Both);
}

/*
fn hex_dump(buf: &[u8]) -> String {
    let vec: Vec<String> = buf.iter().map(|b| format!("{:02x} ", b)).collect();

    vec.join("")
}*/
