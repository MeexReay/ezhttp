use super::{body::{Body, Part}, gen_multipart_boundary, headers::Headers, read_line_crlf, HttpError, Sendable};

use async_trait::async_trait;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::fmt::{Debug, Display};

pub mod status_code {
    pub const OK: &str = "200 OK";
    pub const NOT_FOUND: &str = "404 Not Found";
}

/// Http response
#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status_code: String,
    pub headers: Headers,
    pub body: Body,
}

impl Display for HttpResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self, f)
    }
}

impl HttpResponse {
    pub fn new(
        status_code: &str,
        headers: Headers,
        body: Body
    ) -> Self {
        HttpResponse {
            status_code: status_code.to_string(),
            headers,
            body
        }
    }

    /// Read http response from stream
    pub async fn recv(stream: &mut (impl AsyncReadExt + Unpin)) -> Result<HttpResponse, HttpError> {
        let status = read_line_crlf(stream).await?;

        let (_, status_code) = status.split_once(" ").ok_or(HttpError::InvalidStatus)?;

        let headers = Headers::recv(stream).await?;
        let body = Body::recv(stream, &headers).await?;

        Ok(HttpResponse::new(status_code, headers, body))
    }

    pub fn get_multipart(&self) -> Option<Vec<Part>> {
        let boundary = self.headers.get("content-type")?
            .split(";")
            .map(|o| o.trim())
            .find(|o| o.starts_with("boundary="))
            .map(|o| o[9..].to_string())?;
        Some(self.body.as_multipart(boundary))
    }

    pub fn set_multipart(&mut self, parts: Vec<Part>) -> Option<()> {
        let boundary = gen_multipart_boundary();
        self.headers.put("Content-Type", format!("multipart/form-data; boundary={}", boundary.clone()));
        self.body = Body::from_multipart(parts, boundary);
        Some(())
    }
}

impl Default for HttpResponse {
    
    /// Create new http response with empty headers and data and a 200 OK status code
    fn default() -> Self {
        Self::new("200 OK", Headers::new(), Body::default())
    }
}

#[async_trait]
impl Sendable for HttpResponse {
    /// Write http response to stream
    async fn send(&self, stream: &mut (impl AsyncWriteExt + Unpin + Send)) -> Result<(), HttpError> {
        let mut head: String = String::new();
        head.push_str("HTTP/1.1 ");
        head.push_str(&self.status_code);
        head.push_str("\r\n");
        stream.write_all(head.as_bytes()).await.map_err(|_| HttpError::WriteHeadError)?;

        self.headers.send(stream).await?;

        stream.write_all(b"\r\n").await.map_err(|_| HttpError::WriteBodyError)?;

        self.body.send(stream).await?;

        Ok(())
    }
}
