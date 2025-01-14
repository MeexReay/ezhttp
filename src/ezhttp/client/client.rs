use crate::{error::HttpError, headers::Headers, prelude::HttpResponse, request::HttpRequest};

use super::{send_request, Proxy};

pub struct HttpClient {
    proxy: Proxy,
    verify: bool,
    headers: Headers
}

pub struct ClientBuilder {
    proxy: Proxy,
    verify: bool,
    headers: Headers
}

impl ClientBuilder {
    pub fn new() -> ClientBuilder {
        ClientBuilder {
            proxy: Proxy::None,
            verify: false,
            headers: Headers::new()
        }
    }

    pub fn build(self) -> HttpClient {
        HttpClient { 
            proxy: self.proxy, 
            verify: self.verify, 
            headers: self.headers
        }
    }

    pub fn proxy(mut self, proxy: Proxy) -> Self {
        self.proxy = proxy;
        self
    }

    pub fn verify(mut self, verify: bool) -> Self {
        self.verify = verify;
        self
    }


    pub fn headers(mut self, headers: Headers) -> Self {
        self.headers = headers;
        self
    }

    pub fn header(mut self, name: impl ToString, value: impl ToString) -> Self {
        self.headers.put(name, value.to_string());
        self
    }
}

impl HttpClient {
    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    pub async fn send(&self, request: HttpRequest) -> Result<HttpResponse, HttpError> {
        send_request(request, self.verify, self.proxy.clone(), self.headers.clone()).await
    }
}

impl Default for HttpClient {
    fn default() -> Self {
        ClientBuilder::new().build()
    }
}

