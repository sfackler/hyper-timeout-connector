//! A Hyper `NetworkConnector` which offers a connection timeout.
//!
//! Hyper's default `HttpConnector` does not offer a configurable timeout for
//! the establishment of new connections, so connect calls may block for long
//! periods of time if some addresses don't respond. The `HttpTimeoutConnector` 
//! allows an upper bound to be placed on the time taken by the connection
//! process.
//!
//! # Note
//!
//! The timeout is applied separately to each of the IP addresses associated
//! with the host.
//!
//! # Examples
//!
//! Connecting to HTTP sites:
//!
//! ```
//! extern crate hyper;
//! extern crate hyper_timeout_connector;
//!
//! use hyper::Client;
//! use hyper_timeout_connector::HttpTimeoutConnector;
//! use std::time::Duration;
//!
//! fn main() {
//!     let mut connector = HttpTimeoutConnector::new();
//!     connector.set_connect_timeout(Some(Duration::from_secs(30)));
//!     let client = Client::with_connector(connector);
//!
//!     let response = client.get("http://google.com").send().unwrap();
//! }
//! ```
//!
//! Connecting to HTTPS sites:
//!
//! ```ignore
//! extern crate hyper;
//! extern crate hyper_timeout_connector;
//!
//! use hyper::Client;
//! use hyper::net::HttpsConnector;
//! use hyper_timeout_connector::HttpTimeoutConnector;
//! use std::time::Duration;
//!
//! fn main() {
//!     let mut connector = HttpTimeoutConnector::new();
//!     connector.set_connect_timeout(Some(Duration::from_secs(30)));
//!
//!     let ssl_client = make_ssl_client();
//!     let connector = HttpsConnector::with_connector(ssl_client, connector);
//!     let client = Client::with_connector(connector);
//!
//!     let response = client.get("https://google.com").send().unwrap();
//! }
#![doc(html_root_url="https://docs.rs/hyper-timeout-connector/0.1.0")]
#![warn(missing_docs)]
extern crate hyper;
extern crate socket2;

use hyper::net::{NetworkConnector, HttpStream};
use std::time::Duration;
use std::net::{TcpStream, SocketAddr, ToSocketAddrs};
use socket2::{SockAddr, Socket, Domain, Type};
use std::io;

/// A Hyper `NetworkConnector` which offers a connction timeout.
pub struct HttpTimeoutConnector {
    connect_timeout: Option<Duration>,
}

impl HttpTimeoutConnector {
    /// Creates a new `HttpTimeoutConnector`.
    ///
    /// The connector initially has no connection timeout.
    pub fn new() -> HttpTimeoutConnector {
        HttpTimeoutConnector { connect_timeout: None }
    }

    /// Returns the connection timeout.
    pub fn connect_timeout(&self) -> Option<Duration> {
        self.connect_timeout
    }

    /// Sets the connection timeout.
    pub fn set_connect_timeout(&mut self, timeout: Option<Duration>) {
        self.connect_timeout = timeout;
    }

    fn connect_once(&self, addr: SocketAddr) -> io::Result<TcpStream> {
        let domain = match addr {
            SocketAddr::V4(_) => Domain::ipv4(),
            SocketAddr::V6(_) => Domain::ipv6(),
        };
        let socket = Socket::new(domain, Type::stream(), None)?;
        let addr = SockAddr::from(addr);
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
            match self.connect_once(addr) {
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
