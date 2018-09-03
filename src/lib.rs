#![allow(non_snake_case)]
#![feature(nll)]

extern crate mio;
extern crate slab;

#[macro_use]
extern crate log;

extern crate simple_logger;

use std::net::SocketAddr;
use std::io::{Read, Write};
use mio::{Events, Poll, PollOpt, Ready, Token};
use mio::tcp::{TcpListener};

use slab::Slab;

// have to set the server id to the max usize, as the slab will auto create number one by one
const SERVER_ID: Token = Token(::std::usize::MAX - 1);
const BUF_SIZE: usize = 1024usize;

pub fn run() {

	simple_logger::init_with_level(log::Level::Info).unwrap();

//	let port = setupPort();

	let port = "8989";

	let addr: SocketAddr = format!("127.0.0.1:{}", port).parse()
			.expect("argument format error: port");
	let serverTcpListener = TcpListener::bind(&addr)
			.expect("socket binding error");


	let poll = Poll::new().expect("poll create error");
	poll.register(&serverTcpListener, SERVER_ID, Ready::readable(), PollOpt::edge())
			.expect("poll register error");

	// the event loop
	let mut events = Events::with_capacity(1024);
	let mut tcpStreamSlab = Slab::with_capacity(1024);


	let mut buf = [0u8; BUF_SIZE];
	let stdout = ::std::io::stdout();
	loop {
		poll.poll(&mut events, None).expect("poll error");
		for event in events.iter() {
			let id = event.token();
			if id == SERVER_ID {
				if event.readiness().is_readable() {
					loop{
						let tcpStream = match serverTcpListener.accept() {
							Ok((tcpStream, addr)) =>{
								info!("Accepted tcp stream from {:?}", addr);
								tcpStream
							},
							Err(_) => break
						};
						let streamId = tcpStreamSlab.insert(tcpStream);
						info!("register this stream ID {}", streamId);
						poll.register(&tcpStreamSlab[streamId],
									  Token::from(streamId),
									  Ready::readable(),
									  PollOpt::edge())
								.expect("poll register error");
					}
				}else {
					warn!("the server listener readiness is not readable. will exit");
					::std::process::exit(1);
				}
			}else {
				// if this id is not SERVER, then it must be the one already register before
				// otherwise, it will panic! when the id cannot be found in the slab
				info!("Get ID={:?}", usize::from(id));
				let ref mut stream = tcpStreamSlab[usize::from(id)];
				loop {
					match stream.read(&mut buf) {
						Ok(n) => {
							if n == 0 {
								info!("no data read. mio will auto close connection : {:?}", id);
								info!("removing id from slab");
								tcpStreamSlab.remove(usize::from(id));
								break;
							}else {
								info!("got some data, n={}", n);
								let mut stdOutHandler = stdout.lock();
								stdOutHandler.write(&buf[..n])
										.expect("write to stdout error");
								stdOutHandler.flush()
										.expect("flush to stdout error");
							}
						},
						Err(e) => {
							error!("error when read stream. id:{:?}, error:{}", id, e);
							tcpStreamSlab.remove(usize::from(id));
							break;
						}
					}
				}
			}
		}
	}
}

fn setupPort() -> String {
	let mut args = ::std::env::args();
	let cmd = args.next().unwrap();
	let port = args.next().expect(&format!("Usage: {} [port]", cmd));
	port
}