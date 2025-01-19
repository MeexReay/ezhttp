use std::{collections::HashMap, net::ToSocketAddrs};

use serde_json::Value;

use super::{super::body::{Body, Part}, gen_multipart_boundary, super::headers::Headers, super::request::{HttpRequest, URL}};

/// Builder for [`HttpRequest`](HttpRequest)
pub struct RequestBuilder {
    method: String,
    url: URL,
    headers: Headers,
    body: Option<Body>
}

impl RequestBuilder {
    /// Create builder with a custom method
    pub fn new(method: String, url: URL) -> Self {
        RequestBuilder { 
            method,
            url,
            headers: Headers::new(),
            body: None
        }
    }

    /// Create builder for a GET request
    pub fn get(url: URL) -> Self { Self::new("GET".to_string(), url) }

    /// Create builder for a HEAD request
    pub fn head(url: URL) -> Self { Self::new("HEAD".to_string(), url) }

    /// Create builder for a POST request
    pub fn post(url: URL) -> Self { Self::new("POST".to_string(), url) }

    /// Create builder for a PUT request
    pub fn put(url: URL) -> Self { Self::new("PUT".to_string(), url) }

    /// Create builder for a DELETE request
    pub fn delete(url: URL) -> Self { Self::new("DELETE".to_string(), url) }

    /// Create builder for a CONNECT request
    pub fn connect(url: URL) -> Self { Self::new("CONNECT".to_string(), url) }

    /// Create builder for a OPTIONS request
    pub fn options(url: URL) -> Self { Self::new("OPTIONS".to_string(), url) }

    /// Create builder for a TRACE request
    pub fn trace(url: URL) -> Self { Self::new("TRACE".to_string(), url) }

    /// Create builder for a PATCH request
    pub fn patch(url: URL) -> Self { Self::new("PATCH".to_string(), url) }

    /// Set request url
    pub fn url(mut self, url: URL) -> Self {
        self.url = url;
        self
    }

    /// Set request method
    pub fn method(mut self, method: String) -> Self {
        self.method = method;
        self
    }

    /// Set headers
    pub fn headers(mut self, headers: Headers) -> Self {
        self.headers = headers;
        self
    }

    /// Set header
    pub fn header(mut self, name: impl ToString, value: impl ToString) -> Self {
        self.headers.put(name, value.to_string());
        self
    }

    /// Set body
    pub fn body(mut self, body: Body) -> Self {
        self.body = Some(body);
        self
    }

    /// Set text as body
    pub fn text(mut self, text: impl ToString) -> Self {
        self.body = Some(Body::from_text(text.to_string().as_str()));
        self
    }

    /// Set json as body
    pub fn json(mut self, json: Value) -> Self {
        self.body = Some(Body::from_json(json));
        self
    }

    /// Set raw bytes as body
    pub fn bytes(mut self, bytes: &[u8]) -> Self {
        self.body = Some(Body::from_bytes(bytes));
        self
    }

    /// Set multipart as body
    pub fn multipart(mut self, parts: &[Part]) -> Self {
        let boundary = gen_multipart_boundary();
        self.headers.put("Content-Type", format!("multipart/form-data; boundary={}", boundary.clone()));
        self.body = Some(Body::from_multipart(parts.to_vec(), boundary));
        self
    }

    /// Set query in url
    pub fn url_query(mut self, query: &[(impl ToString, impl ToString)]) -> Self {
        self.url.query = HashMap::from_iter(query.iter().map(|o| (o.0.to_string(), o.1.to_string())));
        self
    }

    /// Set query as body
    pub fn body_query(mut self, query: &[(impl ToString, impl ToString)]) -> Self {
        self.body = Some(Body::from_query(HashMap::from_iter(query.iter().map(|o| (o.0.to_string(), o.1.to_string())))));
        self
    }

    /// Build request
    pub fn build(self) -> HttpRequest {
        HttpRequest { 
            url: self.url, 
            method: self.method,
            addr: "localhost:80".to_socket_addrs().unwrap().next().unwrap(), 
            headers: self.headers, 
            body: self.body.unwrap_or(Body::default())
        }
    }
}