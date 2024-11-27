use std::pin::Pin;

use openssl::ssl::{SslConnector, SslMethod, SslVerifyMode};
use tokio::net::TcpStream;
use tokio_openssl::SslStream;

use super::{error::HttpError, gen_multipart_boundary, headers::Headers, prelude::HttpResponse, request::HttpRequest};


pub mod req_builder;
pub mod client;
pub mod proxy;

pub use req_builder::*;
pub use client::*;
pub use proxy::*;


// TODO: proxy support
async fn send_request(request: HttpRequest, ssl_verify: bool, _proxy: Proxy, headers: Headers) -> Result<HttpResponse, HttpError> {
    let mut request = request;

    let mut stream = TcpStream::connect(
        format!("{}:{}", request.url.domain, request.url.port)
    ).await.map_err(|_| HttpError::ConnectError)?;

    for (key, value) in headers.entries() {
        request.headers.put(key, value);
    }

    request.headers.put("Connection", "close".to_string());
    request.headers.put("Host", request.url.domain.to_string());
    request.headers.put("Content-Length", request.body.as_bytes().len().to_string());

    if request.url.scheme == "http" {
        request.send(&mut stream).await?;

        Ok(HttpResponse::recv(&mut stream).await?)
    } else if request.url.scheme == "https" {
        let mut ssl_connector = SslConnector::builder(SslMethod::tls())
            .map_err(|_| HttpError::SslError)?;
        
        ssl_connector.set_verify(if ssl_verify { SslVerifyMode::PEER }  else { SslVerifyMode::NONE });

        let ssl_connector = ssl_connector.build();

        let ssl = ssl_connector
            .configure()
            .map_err(|_| HttpError::SslError)?
            .into_ssl(&request.url.domain)
            .map_err(|_| HttpError::SslError)?;

        let mut wrapper = SslStream::new(ssl, stream)
            .map_err(|_| HttpError::SslError)?;

        let mut wrapper = Pin::new(&mut wrapper);

        wrapper.as_mut().connect().await.map_err(|_| HttpError::SslError)?;

        request.send(&mut wrapper).await?;

        Ok(HttpResponse::recv(&mut wrapper).await?)
    } else {
        Err(HttpError::UnknownScheme)
    }
}