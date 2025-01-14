use std::pin::Pin;

use base64::Engine;
use openssl::ssl::{SslConnector, SslMethod, SslVerifyMode};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpStream};
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


// TODO: proxy support
async fn send_request(request: HttpRequest, ssl_verify: bool, proxy: Proxy, headers: Headers) -> Result<HttpResponse, HttpError> {
    let mut request = request.clone();

    for (key, value) in headers.entries() {
        request.headers.put(key, value);
    }

    request.headers.put("Connection", "close".to_string());
    request.headers.put("Host", request.url.domain.to_string());
    request.headers.put("Content-Length", request.body.as_bytes().len().to_string());

    let site_host = format!("{}:{}", request.url.domain, request.url.port);

    match proxy {
        Proxy::Http { host, auth } => {
            let mut stream = TcpStream::connect(host).await.map_err(|_| HttpError::ConnectError)?;

            match auth {
                Some((user,password)) => stream.write_all(&[
                    b"CONNECT ", site_host.as_bytes(), b" HTTP/1.1\r\n",
                    b"Host: ", site_host.as_bytes(), b"\r\n",
                    b"Proxy-Authorization: basic ", BASE64_STANDARD.encode(format!("{user}:{password}")).as_bytes(), b"\r\n\r\n",
                ].concat()).await,
                None => stream.write_all(&[
                    b"CONNECT ", site_host.as_bytes(), b" HTTP/1.1\r\n",
                    b"Host: ", site_host.as_bytes(), b"\r\n\r\n",
                ].concat()).await
            }.map_err(|_| HttpError::ConnectError)?;

            HttpResponse::recv(&mut stream).await.map_err(|_| HttpError::ConnectError)?;

            if request.url.scheme == "http" {
                request.send(&mut stream).await?;
        
                Ok(HttpResponse::recv(&mut stream).await?)
            } else if request.url.scheme == "https" {
                let mut wrapper = ssl_wrapper(ssl_verify, request.url.domain.clone(), stream).await?;

                request.send(&mut wrapper).await?;
        
                Ok(HttpResponse::recv(&mut wrapper).await?)
            } else {
                Err(HttpError::UnknownScheme)
            }
        }
        Proxy::Https { host, auth } => {
            let mut stream = TcpStream::connect(host).await.map_err(|_| HttpError::ConnectError)?;

            match auth {
                Some((user,password)) => stream.write_all(&[
                    b"CONNECT ", site_host.as_bytes(), b" HTTP/1.1\r\n",
                    b"Host: ", site_host.as_bytes(), b"\r\n",
                    b"Proxy-Authorization: basic ", BASE64_STANDARD.encode(format!("{user}:{password}")).as_bytes(), b"\r\n\r\n",
                ].concat()).await,
                None => stream.write_all(&[
                    b"CONNECT ", site_host.as_bytes(), b" HTTP/1.1\r\n",
                    b"Host: ", site_host.as_bytes(), b"\r\n\r\n",
                ].concat()).await
            }.map_err(|_| HttpError::ConnectError)?;

            HttpResponse::recv(&mut stream).await.map_err(|_| HttpError::ConnectError)?;

            if request.url.scheme == "http" {
                request.send(&mut stream).await?;
        
                Ok(HttpResponse::recv(&mut stream).await?)
            } else if request.url.scheme == "https" {
                let mut wrapper = ssl_wrapper(ssl_verify, request.url.domain.clone(), stream).await?;

                request.send(&mut wrapper).await?;
        
                Ok(HttpResponse::recv(&mut wrapper).await?)
            } else {
                Err(HttpError::UnknownScheme)
            }
        }
        Proxy::Socks4 { host, user } => {
            let mut stream = match user {
                Some(user) => Socks4Stream::connect_with_userid(host, site_host, &user).await,
                None => Socks4Stream::connect(host, site_host).await
            }.map_err(|_| HttpError::ConnectError)?;

            if request.url.scheme == "http" {
                request.send(&mut stream).await?;
        
                Ok(HttpResponse::recv(&mut stream).await?)
            } else if request.url.scheme == "https" {
                let mut wrapper = ssl_wrapper(ssl_verify, request.url.domain.clone(), stream).await?;

                request.send(&mut wrapper).await?;
        
                Ok(HttpResponse::recv(&mut wrapper).await?)
            } else {
                Err(HttpError::UnknownScheme)
            }
        }
        Proxy::Socks5 { host, auth } => {
            let mut stream = match auth {
                Some(auth) => Socks5Stream::connect_with_password(host, site_host, &auth.0, &auth.1).await,
                None => Socks5Stream::connect(host, site_host).await
            }.map_err(|_| HttpError::ConnectError)?;

            if request.url.scheme == "http" {
                request.send(&mut stream).await?;
        
                Ok(HttpResponse::recv(&mut stream).await?)
            } else if request.url.scheme == "https" {
                let mut wrapper = ssl_wrapper(ssl_verify, request.url.domain.clone(), stream).await?;

                request.send(&mut wrapper).await?;
        
                Ok(HttpResponse::recv(&mut wrapper).await?)
            } else {
                Err(HttpError::UnknownScheme)
            }
        }
        Proxy::None => {
            let mut stream = TcpStream::connect(site_host).await.map_err(|_| HttpError::ConnectError)?;

            if request.url.scheme == "http" {
                request.send(&mut stream).await?;
        
                Ok(HttpResponse::recv(&mut stream).await?)
            } else if request.url.scheme == "https" {
                let mut wrapper = ssl_wrapper(ssl_verify, request.url.domain.clone(), stream).await?;

                request.send(&mut wrapper).await?;
        
                Ok(HttpResponse::recv(&mut wrapper).await?)
            } else {
                Err(HttpError::UnknownScheme)
            }
        }
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