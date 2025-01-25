use std::time::Duration;

use crate::{error::HttpError, headers::Headers, prelude::HttpResponse, request::IntoRequest};

use super::{send_request, Proxy};

/// Client that sends http requests
pub struct HttpClient {
    proxy: Proxy,
    ssl_verify: bool,
    headers: Headers,
    connect_timeout: Option<Duration>, 
    write_timeout: Option<Duration>, 
    read_timeout: Option<Duration>
}

/// [`HttpClient`](HttpClient) builder
pub struct ClientBuilder {
    proxy: Proxy,
    ssl_verify: bool,
    headers: Headers,
    connect_timeout: Option<Duration>, 
    write_timeout: Option<Duration>, 
    read_timeout: Option<Duration>
}

impl ClientBuilder {
    /// Create a client builder
    pub fn new() -> ClientBuilder {
        ClientBuilder {
            proxy: Proxy::None,
            ssl_verify: true,
            headers: Headers::new(),
            connect_timeout: None, 
            write_timeout: None, 
            read_timeout: None
        }
    }

    /// Build a client
    pub fn build(self) -> HttpClient {
        HttpClient { 
            proxy: self.proxy, 
            ssl_verify: self.ssl_verify, 
            headers: self.headers,
            connect_timeout: self.connect_timeout,
            write_timeout: self.write_timeout,
            read_timeout: self.read_timeout
        }
    }

    /// Set request timeouts0
    pub fn timeout(mut self, connect: Option<Duration>, read: Option<Duration>, write: Option<Duration>) -> Self {
        self.connect_timeout = connect;
        self.read_timeout = read;
        self.write_timeout = write;
        self
    }

    /// Set connect timeout
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = Some(timeout);
        self
    }

    /// Set read timeout
    pub fn read_timeout(mut self, timeout: Duration) -> Self {
        self.read_timeout = Some(timeout);
        self
    }

    /// Set write timeout
    pub fn write_timeout(mut self, timeout: Duration) -> Self {
        self.write_timeout = Some(timeout);
        self
    }

    /// Set client proxy
    pub fn proxy(mut self, proxy: Proxy) -> Self {
        self.proxy = proxy;
        self
    }

    /// Set is client have to verify ssl certificate
    pub fn ssl_verify(mut self, verify: bool) -> Self {
        self.ssl_verify = verify;
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
    pub async fn send(&self, request: impl IntoRequest) -> Result<HttpResponse, HttpError> {
        send_request(
            request.to_request()?, 
            self.ssl_verify, 
            self.proxy.clone(), 
            self.headers.clone(),
            self.connect_timeout,
            self.write_timeout,
            self.read_timeout
        ).await
    }

    /// Get connect timeout
    pub fn connect_timeout(&self) -> Option<Duration> {
        self.connect_timeout.clone()
    }

    /// Get read timeout
    pub fn read_timeout(&self) -> Option<Duration> {
        self.read_timeout.clone()
    }

    /// Get write timeout
    pub fn write_timeout(&self) -> Option<Duration> {
        self.write_timeout.clone()
    }

    /// Get client proxy
    pub fn proxy(&self) -> Proxy {
        self.proxy.clone()
    }

    /// Get is client have to verify ssl certificate
    pub fn ssl_verify(&self) -> bool {
        self.ssl_verify
    }

    /// Get default headers
    pub fn headers(&self) -> Headers {
        self.headers.clone()
    }
}

impl Default for HttpClient {
    /// Create default HttpClient
    fn default() -> Self {
        ClientBuilder::new().build()
    }
}

