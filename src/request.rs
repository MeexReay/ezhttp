use super::{body::{Body, Part}, client::RequestBuilder, gen_multipart_boundary, headers::Headers, read_line_crlf, HttpError, Sendable};

use std::{
    collections::HashMap, fmt::{Debug, Display}, net::SocketAddr, str::FromStr
};
use async_trait::async_trait;
use tokio::io::{AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Request URL
#[derive(Clone, Debug)]
pub struct URL {
    pub path: String,
    pub domain: String,
    pub anchor: Option<String>,
    pub query: HashMap<String, String>,
    pub scheme: String,
    pub port: u16
}

impl URL {
    pub fn new(
        domain: String,
        port: u16,
        path: String,
        anchor: Option<String>,
        query: HashMap<String, String>,
        scheme: String
    ) -> URL {
        URL {
            path,
            domain,
            anchor,
            query,
            scheme,
            port
        }
    }

    /// Turns URL object to url string without scheme, domain, port
    /// Example: /123.html?k=v#anc
    pub fn to_path_string(&self) -> String {
        format!("{}{}{}", self.path, if self.query.is_empty() {
            String::new()
        } else {
            "?".to_string()+&self.query.iter().map(|o| {
                format!("{}={}", urlencoding::encode(o.0), urlencoding::encode(o.1))
            }).collect::<Vec<String>>().join("&")
        }, if let Some(anchor) = &self.anchor {
            "#".to_string()+anchor
        } else { 
            String::new()
        })
    }

    /// Turns string without scheme, domain, port to URL object
    /// Example of string: /123.html?k=v#anc
    pub fn from_path_string(s: &str, scheme: String, domain: String, port: u16) -> Option<Self> {
        let (s, anchor) = s.split_once("#").unwrap_or((s, ""));
        let (path, query) = s.split_once("?").unwrap_or((s, ""));

        let anchor = if anchor.is_empty() { None } else { Some(anchor.to_string()) };
        let query = if query.is_empty() { HashMap::new() } else { {
            HashMap::from_iter(query.split("&").filter_map(|entry| {
                let (key, value) = entry.split_once("=").unwrap_or((entry, ""));
                Some((urlencoding::decode(key).ok()?.to_string(), urlencoding::decode(value).ok()?.to_string()))
            }))
        } };
        let path = path.to_string();
        let scheme = scheme.to_string();
        Some(URL { path, domain, anchor, query, scheme, port })
    }
}

impl FromStr for URL {
    type Err = HttpError;

    /// Turns url string to URL object
    /// Example: https://domain.com:999/123.html?k=v#anc
    /// Example 2: http://exampl.eu/sing
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (scheme, s) = s.split_once("://").ok_or(HttpError::UrlError)?;
        let (host, s) = s.split_once("/").unwrap_or((s, ""));
        let (domain, port) = host.split_once(":").unwrap_or((host, 
            if scheme == "http" { "80" } 
            else if scheme == "https" { "443" } 
            else { return Err(HttpError::UrlError) }
        ));
        let port = port.parse::<u16>().map_err(|_| HttpError::UrlError)?;
        let (s, anchor) = s.split_once("#").unwrap_or((s, ""));
        let (path, query) = s.split_once("?").unwrap_or((s, ""));

        let anchor = if anchor.is_empty() { None } else { Some(anchor.to_string()) };
        let query = if query.is_empty() { HashMap::new() } else { {
            HashMap::from_iter(query.split("&").filter_map(|entry| {
                let (key, value) = entry.split_once("=").unwrap_or((entry, ""));
                Some((urlencoding::decode(key).ok()?.to_string(), urlencoding::decode(value).ok()?.to_string()))
            }))
        } };
        let domain = domain.to_string();
        let path = format!("/{path}");
        let scheme = scheme.to_string();
        Ok(URL { path, domain, anchor, query, scheme, port })
    }
}

impl ToString for URL {
    /// Turns URL object to string
    /// Example: https://domain.com:999/123.html?k=v#anc
    /// Example 2: http://exampl.eu/sing
    fn to_string(&self) -> String {
        format!("{}://{}{}", self.scheme, {
            if (self.scheme == "http" && self.port != 80) || (self.scheme == "https" && self.port != 443) {
                format!("{}:{}", self.domain, self.port)
            } else {
                self.domain.clone()
            }
        }, self.to_path_string())
    }
}


/// Http request
#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub url: URL,
    pub method: String,
    pub addr: SocketAddr,
    pub headers: Headers,
    pub body: Body
}

impl Display for HttpRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self, f)
    }
}

impl HttpRequest {
    /// Create new http request
    pub fn new(
        url: URL,
        method: String,
        addr: SocketAddr,
        headers: Headers,
        body: Body
    ) -> Self {
        HttpRequest {
            url,
            method,
            addr,
            headers,
            body
        }
    }

    /// Read http request from stream
    pub async fn recv(stream: &mut (impl AsyncReadExt + Unpin), addr: &SocketAddr) -> Result<HttpRequest, HttpError> {
        let status: Vec<String> = match read_line_crlf(stream).await {
            Ok(i) => {
                i.splitn(3, " ")
                    .map(|s| s.to_string())
                    .collect()
            },
            Err(e) => return Err(e),
        };

        let method = status[0].clone();
        let page = status[1].clone();

        let headers = Headers::recv(stream).await?;
        let body = Body::recv(stream, &headers).await?;

        Ok(HttpRequest::new(
            URL::from_path_string(
                &page, 
                "http".to_string(), 
                "localhost".to_string(), 
                80
            ).ok_or(HttpError::UrlError)?,
            method, 
            addr.clone(), 
            headers, 
            body
        ))
    }

    /// Get multipart parts (requires Content-Type header)
    pub fn get_multipart(&self) -> Option<Vec<Part>> {
        let boundary = self.headers.get("content-type")?
            .split(";")
            .map(|o| o.trim())
            .find(|o| o.starts_with("boundary="))
            .map(|o| o[9..].to_string())?;
        Some(self.body.as_multipart(boundary))
    }

    /// Set multipart parts (modifies Content-Type header)
    pub fn set_multipart(&mut self, parts: Vec<Part>) -> Option<()> {
        let boundary = gen_multipart_boundary();
        self.headers.put("Content-Type", format!("multipart/form-data; boundary={}", boundary.clone()));
        self.body = Body::from_multipart(parts, boundary);
        Some(())
    }

    /// Create new request builder
    pub fn builder(method: String, url: URL) -> RequestBuilder {
        RequestBuilder::new(method, url)
    }
}

#[async_trait]
impl Sendable for HttpRequest {
    async fn send(
        &self,
        stream: &mut (dyn AsyncWrite + Unpin + Send + Sync),
    ) -> Result<(), HttpError> {
        let mut head: String = String::new();
        head.push_str(&self.method);
        head.push_str(" ");
        head.push_str(&self.url.to_path_string());
        head.push_str(" HTTP/1.1");
        head.push_str("\r\n");
        stream.write_all(head.as_bytes()).await.map_err(|_| HttpError::WriteHeadError)?;

        self.headers.send(stream).await?;

        stream.write_all(b"\r\n").await.map_err(|_| HttpError::WriteBodyError)?;

        self.body.send(stream).await?;

        Ok(())
    }
    fn as_box(self) -> Box<dyn Sendable> {
        Box::new(self)
    }
}