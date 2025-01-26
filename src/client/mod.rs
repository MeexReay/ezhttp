use std::{pin::Pin, time::Duration};

use base64::Engine;
use openssl::ssl::{SslConnector, SslMethod, SslVerifyMode};
use tokio::{io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt}, net::TcpStream};
use tokio_io_timeout::TimeoutStream;
use tokio_openssl::SslStream;
use tokio_socks::tcp::{Socks4Stream, Socks5Stream};

use super::{error::HttpError, gen_multipart_boundary, headers::Headers, prelude::HttpResponse, request::HttpRequest, Sendable};

use base64::prelude::BASE64_STANDARD;

pub mod req_builder;
pub mod client;
pub mod proxy;

pub use req_builder::*;
pub use client::*;
pub use proxy::*;

trait RequestStream: AsyncRead + AsyncWrite + Unpin + Send + Sync {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send + Sync> RequestStream for T {}

async fn connect_stream(proxy: Proxy, site_host: &str) -> Result<Box<dyn RequestStream>, HttpError> {
    Ok(match proxy {
        Proxy::Http { host, auth } | Proxy::Https { host, auth } => {
            let mut stream = TcpStream::connect(host).await.map_err(|_| HttpError::ConnectError)?;
            let auth_header = auth.map(|(u, p)| format!("Proxy-Authorization: basic {}\r\n", BASE64_STANDARD.encode(format!("{u}:{p}"))));
            let connect_request = format!("CONNECT {site_host} HTTP/1.1\r\nHost: {site_host}\r\n{}\r\n", auth_header.unwrap_or_default());
            stream.write_all(connect_request.as_bytes()).await.map_err(|_| HttpError::ConnectError)?;
            HttpResponse::recv(&mut stream).await.map_err(|_| HttpError::ConnectError)?;
            Box::new(stream)
        }
        Proxy::Socks4 { host, user } => Box::new(match user {
            Some(user) => Socks4Stream::connect_with_userid(host, site_host, &user).await.map_err(|_| HttpError::ConnectError)?,
            None => Socks4Stream::connect(host, site_host).await.map_err(|_| HttpError::ConnectError)?,
        }),
        Proxy::Socks5 { host, auth } => Box::new(match auth {
            Some((u, p)) => Socks5Stream::connect_with_password(host, site_host, &u, &p).await.map_err(|_| HttpError::ConnectError)?,
            None => Socks5Stream::connect(host, site_host).await.map_err(|_| HttpError::ConnectError)?,
        }),
        Proxy::None => Box::new(TcpStream::connect(site_host).await.map_err(|_| HttpError::ConnectError)?),
    })
}

async fn send_request(
    mut request: HttpRequest, 
    ssl_verify: bool, 
    proxy: Proxy, 
    headers: Headers,
    connect_timeout: Option<Duration>, 
    write_timeout: Option<Duration>, 
    read_timeout: Option<Duration>
) -> Result<HttpResponse, HttpError> {
    for (key, value) in headers.entries() {
        request.headers.put_default(key, value);
    }

    let root = request.clone().url.root.ok_or(HttpError::UrlNeedsRootError)?;

    request.headers.put_default("Connection", "close".to_string());
    request.headers.put_default("Host", root.domain.to_string());
    request.headers.put_default("Content-Length", request.body.as_bytes().len().to_string());
    
    let site_host = format!("{}:{}", root.domain, root.port);
    let stream: Box<dyn RequestStream> = match connect_timeout {
        Some(connect_timeout) => {
            tokio::time::timeout(
                connect_timeout,
                connect_stream(proxy, &site_host)
            ).await.map_err(|_| HttpError::ConnectError)??
        }, None => {
            connect_stream(proxy, &site_host).await?
        }
    };
    
    let mut stream = TimeoutStream::new(stream);
    stream.set_write_timeout(write_timeout);
    stream.set_read_timeout(read_timeout);
    let mut stream = Box::pin(stream);
    
    if root.scheme == "https" {
        let mut stream = ssl_wrapper(ssl_verify, root.domain.clone(), stream).await?;
        request.send(&mut stream).await?;
        Ok(HttpResponse::recv(&mut stream).await?)
    } else {
        request.send(&mut stream).await?;
        Ok(HttpResponse::recv(&mut stream).await?)
    }
}

async fn ssl_wrapper<S: AsyncReadExt + AsyncWriteExt>(ssl_verify: bool, domain: String, stream: S) -> Result<Pin<Box<SslStream<S>>>, HttpError> {
    let mut ssl_connector = SslConnector::builder(SslMethod::tls())
        .map_err(|_| HttpError::SslError)?;
    
    ssl_connector.set_verify(if ssl_verify { SslVerifyMode::PEER } else { SslVerifyMode::NONE });

    let ssl_connector = ssl_connector.build();

    let ssl = ssl_connector
        .configure()
        .map_err(|_| HttpError::SslError)?
        .into_ssl(&domain)
        .map_err(|_| HttpError::SslError)?;

    let wrapper = SslStream::new(ssl, stream)
        .map_err(|_| HttpError::SslError)?;

    let mut wrapper = Box::pin(wrapper);

    wrapper.as_mut().connect().await.map_err(|_| HttpError::SslError)?;

    Ok(wrapper)
}
