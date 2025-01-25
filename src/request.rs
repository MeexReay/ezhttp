use super::{body::{Body, Part}, client::RequestBuilder, gen_multipart_boundary, headers::Headers, read_line_crlf, HttpError, Sendable};

use std::{
    collections::HashMap, fmt::{Debug, Display}, net::SocketAddr, str::FromStr
};
use async_trait::async_trait;
use tokio::io::{AsyncReadExt, AsyncWrite, AsyncWriteExt};


/// Request URL root (scheme://domain:port)
#[derive(Clone, Debug)]
pub struct RootURL {
    pub scheme: String,
    pub domain: String,
    pub port: u16
}

impl FromStr for RootURL {
    type Err = HttpError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (scheme, host) = text.split_once("://").ok_or(HttpError::UrlError)?;
        let (domain, port) = host.split_once(":").unwrap_or(match scheme {
            "https" => (host, "443"),
            "http" => (host, "80"),
            _ => { return Err(HttpError::UrlError) }
        });
        let port = port.parse::<u16>().or(Err(HttpError::UrlError))?;
        let scheme= scheme.to_string();
        let domain= domain.to_string();
        Ok(RootURL { scheme, domain, port })
    }
}

impl ToString for RootURL {
    fn to_string(&self) -> String {
        format!("{}://{}", self.scheme, {
            if (self.scheme == "http" && self.port == 80) || 
                    (self.scheme == "https" && self.port == 443) {
                format!("{}", self.domain)
            } else {
                format!("{}:{}", self.domain, self.port)
            }
        })
    }
}

/// Request URL ({root}/path?query_key=query_value#anchor)
#[derive(Clone, Debug)]
pub struct URL {
    pub root: Option<RootURL>,
    pub path: String,
    pub anchor: Option<String>,
    pub query: HashMap<String, String>
}

impl URL {
    pub fn new(
        root: Option<RootURL>,
        path: String,
        anchor: Option<String>,
        query: HashMap<String, String>,
    ) -> URL {
        URL {
            path,
            anchor,
            query,
            root
        }
    }

    fn to_path_str(&self) -> String {
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

    fn from_path_str(text: &str) -> Option<Self> {
        let (text, anchor) = text.split_once("#").unwrap_or((text, ""));
        let (path, query) = text.split_once("?").unwrap_or((text, ""));
        let path = path.to_string();

        let anchor = if anchor.is_empty() { 
            None 
        } else { 
            Some(anchor.to_string()) 
        };

        let query = if query.is_empty() { 
            HashMap::new() 
        } else {
            HashMap::from_iter(query.split("&").filter_map(|entry| {
                let (key, value) = entry.split_once("=").unwrap_or((entry, ""));
                Some((urlencoding::decode(key).ok()?.to_string(), urlencoding::decode(value).ok()?.to_string()))
            }))
        };

        Some(URL { root: None, path, anchor, query })
    }
}

impl FromStr for URL {
    type Err = HttpError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.starts_with("/") {
            return Self::from_path_str(text).ok_or(HttpError::UrlError)
        }

        let (scheme_n_host, path) = match text.split_once("://") {
            Some((scheme, host_n_path)) => {
                match host_n_path.split_once("/") {
                    Some((host, path)) => {
                        (format!("{}://{}", scheme, host), format!("/{}", path))
                    }, None => {
                        (format!("{}://{}", scheme, host_n_path), "/".to_string())
                    }
                }
            }, None => {
                return Err(HttpError::UrlError)
            }
        };

        let mut url = Self::from_path_str(&path).ok_or(HttpError::UrlError)?;
        url.root = Some(RootURL::from_str(&scheme_n_host)?);

        Ok(url)
    }
}

impl ToString for URL {
    fn to_string(&self) -> String {
        format!("{}{}", self.root.clone().map(|o| o.to_string()).unwrap_or_default(), self.to_path_str())
    }
}

pub trait IntoURL: ToString {
    fn to_url(self) -> Result<URL, HttpError>;
}

impl IntoURL for &String {
    fn to_url(self) -> Result<URL, HttpError> {
        URL::from_str(&self)
    }
}

impl IntoURL for String {
    fn to_url(self) -> Result<URL, HttpError> {
        URL::from_str(&self)
    }
}

impl IntoURL for &str {
    fn to_url(self) -> Result<URL, HttpError> {
        URL::from_str(&self)
    }
}

impl IntoURL for URL {
    fn to_url(self) -> Result<URL, HttpError> {
        Ok(self)
    }
}

pub trait IntoRequest {
    fn to_request(self) -> Result<HttpRequest, HttpError>;
}

impl IntoRequest for HttpRequest {
    fn to_request(self) -> Result<HttpRequest, HttpError> {
        Ok(self)
    }
}

/// Http request
#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub url: URL,
    pub method: String,
    pub addr: Option<SocketAddr>,
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
        url: impl IntoURL,
        method: String,
        headers: Headers,
        body: Body,
        addr: Option<SocketAddr>
    ) -> Result<Self, HttpError> {
        Ok(HttpRequest {
            url: url.to_url()?,
            method,
            headers,
            body,
            addr
        })
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

        HttpRequest::new(
            page,
            method, 
            headers, 
            body,
            Some(addr.clone())
        )
    }

    /// Get multipart parts (requires Content-Type header)
    pub fn get_multipart(&self) -> Option<Vec<Part>> {
        let boundary = self.headers.get("content-type").get(0)?
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
        let mut url = self.url.clone();
        url.root = None;

        let mut head: String = String::new();
        head.push_str(&self.method);
        head.push_str(" ");
        head.push_str(&url.to_string());
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