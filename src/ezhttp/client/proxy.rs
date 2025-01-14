use std::net::{ToSocketAddrs, SocketAddr};

#[derive(Clone, Debug)]
pub enum Proxy {
    None,
    Socks5 { host: SocketAddr, auth: Option<(String, String)> },
    Socks4 { host: SocketAddr, user: Option<String> },
    Http { host: SocketAddr, auth: Option<(String, String)> },
    Https { host: SocketAddr, auth: Option<(String, String)> },
}

impl Proxy {
    pub fn none() -> Self {
        Self::None
    }

    pub fn socks5(host: impl ToSocketAddrs) -> Self {
        Self::Socks5 { host: host.to_socket_addrs().unwrap().next().unwrap(), auth: None }
    }

    pub fn socks5_with_auth(host: impl ToSocketAddrs, user: String, password: String) -> Self {
        Self::Socks5 { host: host.to_socket_addrs().unwrap().next().unwrap(), auth: Some((user, password)) }
    }

    pub fn socks4(host: impl ToSocketAddrs) -> Self {
        Self::Socks4 { host: host.to_socket_addrs().unwrap().next().unwrap(), user: None }
    }

    pub fn socks4_with_auth(host: impl ToSocketAddrs, user_id: String) -> Self {
        Self::Socks4 { host: host.to_socket_addrs().unwrap().next().unwrap(), user: Some(user_id) }
    }

    pub fn http(host: impl ToSocketAddrs) -> Self {
        Self::Http { host: host.to_socket_addrs().unwrap().next().unwrap(), auth: None }
    }

    pub fn http_with_auth(host: impl ToSocketAddrs, user: String, password: String) -> Self {
        Self::Http { host: host.to_socket_addrs().unwrap().next().unwrap(), auth: Some((user, password)) }
    }

    pub fn https(host: impl ToSocketAddrs) -> Self {
        Self::Https { host: host.to_socket_addrs().unwrap().next().unwrap(), auth: None }
    }

    pub fn https_with_auth(host: impl ToSocketAddrs, user: String, password: String) -> Self {
        Self::Https { host: host.to_socket_addrs().unwrap().next().unwrap(), auth: Some((user, password)) }
    }
}