use crate::{error::HttpError, headers::Headers, prelude::HttpResponse, request::HttpRequest};

use super::{send_request, Proxy};

/// Client that sends http requests
pub struct HttpClient {
    proxy: Proxy,
    verify: bool,
    headers: Headers
}

/// [`HttpClient`](HttpClient) builder
pub struct ClientBuilder {
    proxy: Proxy,
    verify: bool,
    headers: Headers
}

impl ClientBuilder {
    /// Create a client builder
    pub fn new() -> ClientBuilder {
        ClientBuilder {
            proxy: Proxy::None,
            verify: false,
            headers: Headers::new()
        }
    }

    /// Build a client
    pub fn build(self) -> HttpClient {
        HttpClient { 
            proxy: self.proxy, 
            verify: self.verify, 
            headers: self.headers
        }
    }

    /// Set client proxy
    pub fn proxy(mut self, proxy: Proxy) -> Self {
        self.proxy = proxy;
        self
    }

    /// Set is client have to verify ssl certificate
    pub fn verify(mut self, verify: bool) -> Self {
        self.verify = verify;
        self
    }

    /// Set default headers
    pub fn headers(mut self, headers: Headers) -> Self {
        self.headers = headers;
        self
    }

    /// Set default header
    pub fn header(mut self, name: impl ToString, value: impl ToString) -> Self {
        self.headers.put(name, value.to_string());
        self
    }
}

impl HttpClient {
    /// Get new HttpClient builder
    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    /// Sends a request and receives a response
    pub async fn send(&self, request: HttpRequest) -> Result<HttpResponse, HttpError> {
        send_request(request, self.verify, self.proxy.clone(), self.headers.clone()).await
    }
}

impl Default for HttpClient {
    /// Create default HttpClient
    fn default() -> Self {
        ClientBuilder::new().build()
    }
}

