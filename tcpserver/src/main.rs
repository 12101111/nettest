use anyhow::{anyhow, Result};
use rand::{Rng, SeedableRng};
use rand_xoshiro::Xoshiro256Plus;
use std::io::{BufRead, BufReader, Write};
use std::net::{self, SocketAddr, TcpListener, TcpStream};
const MB: usize = 1024 * 1024;

fn main() {
    let mut args = std::env::args();
    let cmd = args.next().unwrap();
    if args.len() != 1 {
        println!("Usage: `{} [PORT]", cmd);
        return;
    }
    let port: u16 = args.next().unwrap().parse().unwrap();
    let socker = SocketAddr::new(net::Ipv4Addr::UNSPECIFIED.into(), port);
    if let Err(e) = server(socker) {
        eprintln!("Err: {}", e);
    }
}

fn server(addr: SocketAddr) -> Result<()> {
    let rand_pool: Vec<u8> = {
        let mut rng = Xoshiro256Plus::from_entropy();
        let mut buffer = Vec::with_capacity(2 * MB);
        for _ in 0..2 * MB {
            buffer.push(rng.gen_range(0x20, 0x7F));
        }
        buffer
    };
    let listener = TcpListener::bind(addr)?;
    let mut incoming = listener.incoming();
    while let Some(stream) = incoming.next() {
        let stream = match stream {
            Err(e) => {
                eprintln!("TCP client connection error: {}", e);
                continue;
            }
            Ok(s) => s,
        };
        if let Err(e) = handle_stream(stream, &rand_pool) {
            eprintln!("TCP client handle error: {}", e);
        }
    }
    Ok(())
}

enum HandleState {
    Next,
    Quit,
}

fn handle_stream(mut stream: TcpStream, rand_pool: &[u8]) -> Result<()> {
    loop {
        let mut reader = BufReader::new(stream);
        let mut buf = String::new();
        reader.read_line(&mut buf)?;
        stream = reader.into_inner();
        match handle_inner(&mut stream, &buf, rand_pool) {
            Err(e) => {
                eprintln!("Tcp client {:?} error: {:?}", stream, e);
                stream.write_all(b"ERROR\n")?;
                break;
            }
            Ok(HandleState::Quit) => break,
            Ok(HandleState::Next) => continue,
        }
    }
    println!("Disconnect with {}", stream.peer_addr()?);
    let _ = stream.shutdown(std::net::Shutdown::Both);
    Ok(())
}

fn handle_inner(stream: &mut TcpStream, buf: &str, rand_pool: &[u8]) -> Result<HandleState> {
    match buf {
        _ if buf.split_whitespace().next().is_none() => Ok(HandleState::Next),
        _ if buf.starts_with("QUIT") => Ok(HandleState::Quit),
        _ if buf.starts_with("DOWNLOAD ") => handle_download(stream, &buf, rand_pool),
        _ if buf.starts_with("UPLOAD ") => handle_upload(stream, &buf),
        _ if buf.starts_with("GETIP") => {
            let peer_ip = stream.peer_addr()?.ip();
            let resp = format!("YOURIP {}\n", peer_ip);
            stream.write_all(resp.as_bytes())?;
            //print!("{}", resp);
            Ok(HandleState::Next)
        }
        _ if buf.starts_with("PING ") => {
            let time_stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let resp = format!("PONG {}\n", time_stamp);
            stream.write_all(resp.as_bytes())?;
            //print!("{}", resp);
            Ok(HandleState::Next)
        }
        _ if buf.starts_with("HI") => {
            stream.write_all(b"HELLO 2.7(compatiable server)\n")?;
            Ok(HandleState::Next)
        }
        _ if buf.starts_with("CAPABILITIES") => {
            stream.write_all(b"CAPABILITIES SERVER_HOST_AUTH UPLOAD_STATS\n")?;
            Ok(HandleState::Next)
        }
        _ => Err(anyhow!("unknown command")),
    }
}

fn handle_download(stream: &mut TcpStream, buf: &str, rand_pool: &[u8]) -> Result<HandleState> {
    let download_bytes: usize = buf[8..].trim().parse()?;
    const START: &[u8] = b"DOWNLOAD ";
    const END: &[u8] = b"\r\n";
    if download_bytes <= 1 {
        stream.write_all(END)?;
    } else if download_bytes <= 11 {
        stream.write_all(&START[0..download_bytes - 2])?;
        stream.write_all(END)?;
    } else {
        stream.write_all(START)?;
        let mut size = download_bytes - 11;
        let mut rng = Xoshiro256Plus::from_entropy();
        while size > 0 {
            let start = rng.gen_range(0, MB);
            let len = size.min(MB);
            stream.write_all(&rand_pool[start..start + len])?;
            size -= len;
        }
        stream.write_all(END)?;
    }
    Ok(HandleState::Next)
}

fn handle_upload(mut stream: &mut TcpStream, buf: &str) -> Result<HandleState> {
    let upload_bytes = buf
        .split_whitespace()
        .nth(1)
        .ok_or(anyhow!("Upload command don't have bytes value"))?
        .parse::<usize>()?;
    let mut reader = BufReader::with_capacity(MB, stream);
    let mut length = buf.len();
    loop {
        let buffer = reader.fill_buf()?;
        let len = dbg!(buffer.len());
        length += len;
        if length >= upload_bytes || len == 0 {
            dbg!(buffer.last());
            break;
        }
        reader.consume(len);
    }
    stream = reader.into_inner();
    let time_stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let ans = format!("Ok {} {}\n", length - 1, time_stamp);
    stream.write_all(dbg!(ans).as_bytes())?;
    Ok(HandleState::Next)
}

fn hex_dump(buf: &[u8]) -> String {
    let vec: Vec<String> = buf.iter().map(|b| format!("{:02x} ", b)).collect();

    vec.join("")
}
