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
use mio::tcp::TcpListener;
use std::borrow::Cow;


use slab::Slab;

// have to set the server id to the max usize, as the slab will auto create number one by one
const SERVER_ID: Token = Token(::std::usize::MAX - 1);
const BUF_SIZE: usize = 1024usize;
const HTTP_RESP: &str = "HTTP/1.1 200 OK
Content-Type: text/html; charset=UTF-8

<html>
      <head></head>
      <body>
            <h1>This is Response</h1>
      </body>
</html>";

pub fn run() {
	simple_logger::init_with_level(log::Level::Info).unwrap();

	let port = setupPort("8989");


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
			match event.token() {
				SERVER_ID => {
					if event.readiness().is_readable() {
						loop {
							let tcpStream = match serverTcpListener.accept() {
								Ok((tcpStream, addr)) => {
									info!("Accepted tcp stream from {:?}", addr);
									tcpStream
								}
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
					} else {
						warn!("the server listener readiness is not readable. will exit");
						::std::process::exit(1);
					}
				}
				Token(id) => {
					// if this id is not SERVER, then it must be the one already register before
					// otherwise, it will panic! when the id cannot be found in the slab
					info!("Get event. ID={:?}", usize::from(id));
					let ref mut stream = tcpStreamSlab[usize::from(id)];

					if event.readiness().is_readable() {
						loop {
							match stream.read(&mut buf) {
								Ok(n) => {
									if n == 0 {
										info!("no data read. mio will auto close connection : {:?}", id);
										info!("removing id from slab");
										tcpStreamSlab.remove(usize::from(id));
										break;
									} else {
										info!("Receive {} Bytes", n);
										let mut stdOutHandler = stdout.lock();
										let s = ::std::str::from_utf8(&buf).unwrap();
										let mut ss = String::new();
										ss.push_str(s);
										stdOutHandler.write(&buf[..n])
												.expect("write to stdout error");
										stdOutHandler.write("\n".as_bytes())
												.expect("write to stdout error");
										stdOutHandler.flush()
												.expect("flush to stdout error");

									}
								}
								Err(e) => {
									error!("error when read stream. id:{:?}, error:{}", id, e);
									info!("going to re-register the write event");
									// after read. we can wait for the write
									poll.reregister(stream,
									                Token::from(id),
									                Ready::writable(),
									                PollOpt::edge())
											.unwrap();
									break;
								}
							}
						}
					} else if event.readiness().is_writable() {
						let mut stream = tcpStreamSlab.remove(usize::from(id));
						match stream.write(HTTP_RESP.as_bytes()) {
							Ok(nWrite) => info!("Response {} Bytes", nWrite),
							Err(e) => error!("Response error: {}", e),
						}
						stream.flush().unwrap();
						break;
					} else {
						let readiness = event.readiness();
						warn!("neither writable or readable. the readiness is {:?}", readiness);
					}
				}
			}
		}
	}
}

fn setupPort(defaultPort: &str) -> Cow<str> {
	let mut args = ::std::env::args();
	let _cmd = args.next().unwrap();
	let port = match args.next() {
		Some(s) => match s.parse::<u8>() {
			Ok(_) => Cow::Owned(s),
			Err(_) => Cow::Borrowed(defaultPort),
		}
		None => Cow::Borrowed(defaultPort),
	};
	info!("Use port: {}", port);
	return port;
}