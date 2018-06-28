#![allow(non_snake_case)]
#![feature(nll)]
extern crate mio;
extern crate slab;

use std::net::SocketAddr;
use std::io::{Read, Write, ErrorKind};
use mio::*;
use mio::tcp::TcpListener;
use mio::Token;

use slab::Slab;

//type Slab<T> = slab::Slab<T, Token>;

const THIS_CONN_ID: Token = Token(::std::usize::MAX - 1);

fn main() {

  let mut args = ::std::env::args();
  let cmd = args.next().unwrap();
  let port = args.next().expect(&format!("Usage: {} [port]", cmd));

  let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().expect("argument format error: port");
  let serverTcpListener = TcpListener::bind(&addr).expect("socket binding error");

  let poll = Poll::new().expect("poll create error");
  poll.register(&serverTcpListener, THIS_CONN_ID, Ready::readable(), PollOpt::edge())
      .expect("poll register error");

  // the event loop
  let mut events = Events::with_capacity(1024);
  let mut tcpStreamMap = Slab::with_capacity(1024);
  let mut buf: [u8; 1024] = [0; 1024];
  let stdout = ::std::io::stdout();
  loop {
    poll.poll(&mut events, None).expect("poll error");
    for event in events.iter() {
      let (connectionId, readiness) = (event.token(), event.readiness());
      if readiness.is_readable() {
        if connectionId == THIS_CONN_ID {
          loop {
            // the tcpListener is linked with the connection id
            let tcpStream = match serverTcpListener.accept() {
              Ok((tcpStream, addr)) => {
                println!("Accepted connection: {}", addr);
                tcpStream
              }
              Err(_) => break
            };
            let tcpStreamId = tcpStreamMap.insert(tcpStream);
            poll.register(&tcpStreamMap[tcpStreamId], Token::from(tcpStreamId), Ready::readable(), PollOpt::edge())
                .expect("poll register error");
          }
        }else {
          let ref mut tcpStream = tcpStreamMap[usize::from(connectionId)];
          loop {
            match tcpStream.read(&mut buf) { // read into buffer
              Ok(n) => {
                if n == 0 {
                  println!("Closing connection on token={:?}", connectionId);
                  tcpStreamMap.remove(usize::from(connectionId));
                  break;
                } else {
                  let mut stdoutHandle = stdout.lock();
                  stdoutHandle.write(&buf[..n]).expect("write error"); // write from buffer
                  stdoutHandle.flush().expect("flush error");
                }
              }
              Err(e) => {
                if e.kind() != ErrorKind::WouldBlock {
                  println!("Closing connection on token={:?}", connectionId);
                  tcpStreamMap.remove(usize::from(connectionId));
                }
                break;
              }
            }
          }
        }
      }else {
        println!("Event readiness is not readable");
        if connectionId == THIS_CONN_ID {
          ::std::process::exit(1);
        }
        tcpStreamMap.remove(usize::from(connectionId));
      }
    }
  }
}