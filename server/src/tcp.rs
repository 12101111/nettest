use super::MB;
use anyhow::{anyhow, Result};
use async_std::io::{BufReader, Read};
use async_std::net::{TcpListener, TcpStream};
use async_std::prelude::*;
use rand::{Rng, SeedableRng};
use rand_xoshiro::Xoshiro256Plus;
use std::net::SocketAddr;

pub async fn server(addr: SocketAddr) -> Result<()> {
    let rand_pool: Vec<u8> = {
        let mut rng = Xoshiro256Plus::from_entropy();
        let mut buffer = Vec::with_capacity(2 * MB);
        for _ in 0..2 * MB {
            buffer.push(rng.gen_range(0x20, 0x7F));
        }
        buffer
    };
    let listener = TcpListener::bind(addr).await?;
    let mut incoming = listener.incoming();
    while let Some(stream) = incoming.next().await {
        let stream = match stream {
            Err(e) => {
                eprintln!("TCP client connection error: {}", e);
                continue;
            }
            Ok(s) => s,
        };
        if let Err(e) = handle_stream(stream, &rand_pool).await {
            eprintln!("TCP client handle error: {}", e);
        }
    }
    Ok(())
}

enum HandleState {
    Next,
    Quit,
}

async fn handle_stream(mut stream: TcpStream, rand_pool: &[u8]) -> Result<()> {
    loop {
        let mut reader = BufReader::new(stream);
        let mut buf = String::new();
        reader.read_line(&mut buf).await?;
        stream = reader.into_inner();
        match handle_inner(&mut stream, &buf, rand_pool).await {
            Err(e) => {
                eprintln!("Tcp client {:?} error: {:?}", stream, e);
                stream.write_all(b"ERROR\n").await?;
                break;
            }
            Ok(HandleState::Quit) => break,
            Ok(HandleState::Next) => continue,
        }
    }
    println!("Disconnect with {}",stream.peer_addr()?);
    let _ = stream.shutdown(std::net::Shutdown::Both);
    Ok(())
}

async fn handle_inner(stream: &mut TcpStream, buf: &str, rand_pool: &[u8]) -> Result<HandleState> {
    match buf {
        _ if buf.split_whitespace().next().is_none() => Ok(HandleState::Next),
        _ if buf.starts_with("QUIT") => Ok(HandleState::Quit),
        _ if buf.starts_with("DOWNLOAD ") => handle_download(stream, &buf, rand_pool).await,
        _ if buf.starts_with("UPLOAD ") => handle_upload(stream, &buf).await,
        _ if buf.starts_with("GETIP") => {
            let peer_ip = stream.peer_addr()?.ip();
            let resp = format!("YOURIP {}\n", peer_ip);
            stream.write_all(resp.as_bytes()).await?;
            //print!("{}", resp);
            Ok(HandleState::Next)
        }
        _ if buf.starts_with("PING ") => {
            let time_stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let resp = format!("PONG {}\n", time_stamp);
            stream.write_all(resp.as_bytes()).await?;
            //print!("{}", resp);
            Ok(HandleState::Next)
        }
        _ if buf.starts_with("HI") => {
            stream.write_all(b"HELLO 2.7(compatiable server)\n").await?;
            Ok(HandleState::Next)
        }
        _ if buf.starts_with("CAPABILITIES") => {
            stream
                .write_all(b"CAPABILITIES SERVER_HOST_AUTH UPLOAD_STATS\n")
                .await?;
            Ok(HandleState::Next)
        }
        _ => Err(anyhow!("unknown command")),
    }
}

async fn handle_download(
    stream: &mut TcpStream,
    buf: &str,
    rand_pool: &[u8],
) -> Result<HandleState> {
    let download_bytes: usize = buf[8..].trim().parse()?;
    const START: &[u8] = b"DOWNLOAD ";
    const END: &[u8] = b"\r\n";
    if download_bytes <= 1 {
        stream.write_all(END).await?;
    } else if download_bytes <= 11 {
        stream.write_all(&START[0..download_bytes - 2]).await?;
        stream.write_all(END).await?;
    } else {
        stream.write_all(START).await?;
        let mut size = download_bytes - 11;
        let mut rng = Xoshiro256Plus::from_entropy();
        while size > 0 {
            let start = rng.gen_range(0, MB);
            let len = size.min(MB);
            stream.write_all(&rand_pool[start..start + len]).await?;
            size -= len;
        }
        stream.write_all(END).await?;
    }
    Ok(HandleState::Next)
}

async fn handle_upload(mut stream: &mut TcpStream, buf: &str) -> Result<HandleState> {
    let upload_bytes = buf
        .split_whitespace()
        .nth(1)
        .ok_or(anyhow!("Upload command don't have bytes value"))?
        .parse::<usize>()?
        + 1;
    let count = Count {
        reader: &mut stream,
        buffer: &mut vec![0u8; upload_bytes.min(MB)],
        count: 0,
    };
    let size = count.await? + buf.as_bytes().len();
    let time_stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let ans = format!("Ok {} {}\n", size, time_stamp);
    stream.write_all(dbg!(ans).as_bytes()).await?;
    Ok(HandleState::Next)
}

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

struct Count<'a, R: ?Sized> {
    reader: &'a mut R,
    buffer: &'a mut [u8],
    count: usize,
}

impl<A> Future for Count<'_, A>
where
    A: Read + ?Sized + Unpin,
{
    type Output = std::io::Result<usize>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let mut rd = Pin::new(&mut this.reader);
        loop {
            let n = match rd.as_mut().poll_read(cx, this.buffer) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Ready(Ok(n)) => n,
            };
            dbg!([this.count,n]);
            if n == 0 {
                return Poll::Ready(Ok(this.count));
            }else{
                this.count += n;
                let last = this.buffer[n-1];
                if last == b'\n'{
                    return Poll::Ready(Ok(this.count));
                }
            }
        }
    }
}

/*
struct Count<R: Read + Unpin> {
    reader: BufReader<R>,
    count: usize,
}

impl<R: Read + Unpin> Future for Count<R> {
    type Output = async_std::io::Result<usize>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        loop {
            let buf = futures_core::ready!(Pin::new(&mut this.reader).poll_fill_buf(cx))?;
            let len = buf.len();
            if len == 0 || buf.last() == Some(&b'\n') {
                return Poll::Ready(Ok(this.count));
            }
            Pin::new(&mut this.reader).consume(len);
            this.count += len;
            dbg!(len);
        }
    }
}*/
