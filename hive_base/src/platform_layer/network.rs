use std::io;
use std::net::{TcpStream, TcpListener, UdpSocket, ToSocketAddrs};

/// Cross-platform TCP connect
pub fn tcp_connect<A: ToSocketAddrs>(addr: A) -> io::Result<TcpStream> {
    TcpStream::connect(addr)
}

/// Cross-platform TCP bind + listen
pub fn tcp_bind<A: ToSocketAddrs>(addr: A, _backlog: u32) -> io::Result<TcpListener> {
    let listener = TcpListener::bind(addr)?;
    // Non-blocking accept for C2 compatibility
    listener.set_nonblocking(true)?;
    Ok(listener)
}

/// Cross-platform UDP socket
pub fn udp_bind<A: ToSocketAddrs>(addr: A) -> io::Result<UdpSocket> {
    let socket = UdpSocket::bind(addr)?;
    Ok(socket)
}

/// Resolve hostname to IP addresses
pub fn resolve_hostname(host: &str) -> io::Result<Vec<std::net::IpAddr>> {
    let addrs: Vec<std::net::IpAddr> = (host, 0)
        .to_socket_addrs()?
        .map(|sa| sa.ip())
        .collect();
    if addrs.is_empty() {
        Err(io::Error::new(io::ErrorKind::NotFound, "no addresses resolved"))
    } else {
        Ok(addrs)
    }
}
