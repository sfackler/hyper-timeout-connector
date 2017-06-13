extern crate hyper;
extern crate socket2;

use hyper::net::{NetworkConnector, HttpStream};
use std::time::Duration;
use std::net::{TcpStream, SocketAddr, ToSocketAddrs};
use socket2::{SockAddr, Socket, Domain, Type};
use std::io;

pub struct HttpTimeoutConnector {
    connect_timeout: Option<Duration>,
}

impl HttpTimeoutConnector {
    pub fn new() -> HttpTimeoutConnector {
        HttpTimeoutConnector { connect_timeout: None }
    }

    pub fn connect_timeout(&self) -> Option<Duration> {
        self.connect_timeout
    }

    pub fn set_connect_timeout(&mut self, timeout: Option<Duration>) {
        self.connect_timeout = timeout;
    }

    fn connect_once(&self, addr: &SocketAddr) -> io::Result<TcpStream> {
        let domain = match *addr {
            SocketAddr::V4(_) => Domain::ipv4(),
            SocketAddr::V6(_) => Domain::ipv6(),
        };
        let socket = Socket::new(domain, Type::stream(), None)?;
        let addr = SockAddr::from(*addr);
        match self.connect_timeout {
            Some(timeout) => socket.connect_timeout(&addr, timeout)?,
            None => socket.connect(&addr)?,
        }

        Ok(socket.into())
    }
}

impl NetworkConnector for HttpTimeoutConnector {
    type Stream = HttpStream;

    fn connect(&self, host: &str, port: u16, scheme: &str) -> hyper::Result<HttpStream> {
        if scheme != "http" {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid scheme for http")
                           .into());
        }

        let mut last_err = None;
        for addr in (host, port).to_socket_addrs()? {
            match self.connect_once(&addr) {
                Ok(l) => return Ok(HttpStream(l)),
                Err(e) => last_err = Some(e),
            }
        }

        Err(last_err
                .unwrap_or_else(|| {
                                    io::Error::new(io::ErrorKind::InvalidInput,
                                                   "could not resolve to any addresses")
                                })
                .into())
    }
}

#[cfg(test)]
mod test {
    use hyper::{self, Client};

    use super::*;

    #[test]
    fn timeout() {
        let mut connector = HttpTimeoutConnector::new();
        connector.set_connect_timeout(Some(Duration::from_millis(250)));

        let client = Client::with_connector(connector);

        // this is an unroutable IP so connection should always time out
        match client.get("http://10.255.255.1:80").send() {
            Ok(_) => panic!("unexpected success"),
            Err(hyper::Error::Io(ref e)) if e.kind() == io::ErrorKind::TimedOut => {}
            Err(e) => panic!("unexpected error {:?}", e),
        }
    }

    #[test]
    fn ok() {
        let mut connector = HttpTimeoutConnector::new();
        connector.set_connect_timeout(Some(Duration::from_millis(250)));

        let client = Client::with_connector(connector);
        client.get("http://google.com").send().unwrap();
    }
}
