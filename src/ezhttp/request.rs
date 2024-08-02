use super::{read_line_crlf, read_line_lf, rem_first, split, Headers, HttpError};

use serde_json::Value;
use std::{
    fmt::{Debug, Display},
    io::{Read, Write},
    net::{IpAddr, SocketAddr, ToSocketAddrs},
};

/// Http request
#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub page: String,
    pub method: String,
    pub addr: String,
    pub headers: Headers,
    pub params: Value,
    pub data: Vec<u8>,
}

impl Display for HttpRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self, f)
    }
}

impl HttpRequest {
    /// Create new http request
    pub fn new(page: &str, method: &str, params: Value, headers: Headers, data: Vec<u8>) -> Self {
        HttpRequest {
            page: page.to_string(),
            method: method.to_string(),
            addr: String::new(),
            params,
            headers,
            data,
        }
    }

    /// Read http request from stream
    pub fn read(data: &mut impl Read, addr: &SocketAddr) -> Result<HttpRequest, HttpError> {
        let octets = match addr.ip() {
            IpAddr::V4(ip) => ip.octets(),
            _ => [127, 0, 0, 1],
        };

        let ip_str = octets[0].to_string()
            + "."
            + &octets[1].to_string()
            + "."
            + &octets[2].to_string()
            + "."
            + &octets[3].to_string();

        let status = split(
            match read_line_crlf(data) {
                Ok(i) => i,
                Err(e) => return Err(e),
            },
            " ",
            3,
        );

        let method = status[0].clone();
        let (page, query) = match status[1].split_once("?") {
            Some(i) => (i.0.to_string(), Some(i.1)),
            None => (status[1].clone(), None),
        };

        let mut headers = Headers::new();

        loop {
            let text = match read_line_crlf(data) {
                Ok(i) => i,
                Err(_) => return Err(HttpError::InvalidHeaders),
            };

            if text.len() == 0 {
                break;
            }

            let (key, value) = match text.split_once(": ") {
                Some(i) => i,
                None => return Err(HttpError::InvalidHeaders),
            };

            headers.put(key.to_lowercase(), value.to_string());
        }

        let mut params = serde_json::Map::new();

        if let Some(i) = query {
            for ele in i.split("&") {
                let (k, v) = match ele.split_once("=") {
                    Some(i) => i,
                    None => return Err(HttpError::InvalidQuery),
                };

                params.insert(
                    match urlencoding::decode(k) {
                        Ok(i) => i.to_string(),
                        Err(_) => return Err(HttpError::InvalidQuery),
                    },
                    match urlencoding::decode(v) {
                        Ok(i) => Value::String(i.to_string()),
                        Err(_) => return Err(HttpError::InvalidQuery),
                    },
                );
            }
        }

        let mut reqdata: Vec<u8> = Vec::new();

        if let Some(content_size) = headers.clone().get("content-length".to_string()) {
            let content_size: usize = match content_size.parse() {
                Ok(i) => i,
                Err(_) => return Err(HttpError::InvalidContentSize),
            };

            if content_size > reqdata.len() {
                let mut buf: Vec<u8> = Vec::new();
                buf.resize(content_size - reqdata.len(), 0);

                match data.read_exact(&mut buf) {
                    Ok(i) => i,
                    Err(_) => return Err(HttpError::InvalidContent),
                };

                reqdata.append(&mut buf);
            }
        }

        if let Some(content_type) = headers.clone().get("content-type".to_string()) {
            let mut body = match String::from_utf8(reqdata.clone()) {
                Ok(i) => i,
                Err(_) => return Err(HttpError::InvalidContent),
            };

            match content_type.as_str() {
                "application/json" => {
                    let val: Value = match serde_json::from_str(&body) {
                        Ok(i) => i,
                        Err(_) => return Err(HttpError::JsonParseError),
                    };

                    if let Value::Object(mut dict) = val {
                        params.append(&mut dict);
                    }
                }
                "multipart/form-data" => {
                    let boundary = "--".to_string()
                        + &content_type[(content_type.find("boundary=").unwrap() + 9)..]
                        + "\r\n";
                    for part in body.split(boundary.as_str()) {
                        let lines: Vec<&str> = part.split("\r\n").collect();
                        if lines.len() >= 3 {
                            if lines[0].starts_with("Content-Disposition: form-data; name=\"") {
                                let name: &str =
                                    &lines[0]["Content-Disposition: form-data; name=\"".len()..];
                                let name: &str = &name[..name.len() - 1];
                                params
                                    .insert(name.to_string(), Value::String(lines[2].to_string()));
                            }
                        }
                    }
                }
                "application/x-www-form-urlencoded" => {
                    if body.starts_with("?") {
                        body = rem_first(body.as_str()).to_string()
                    }

                    for ele in body.split("&") {
                        let (k, v) = match ele.split_once("=") {
                            Some(i) => i,
                            None => return Err(HttpError::InvalidQuery),
                        };

                        params.insert(
                            match urlencoding::decode(k) {
                                Ok(i) => i.to_string(),
                                Err(_) => return Err(HttpError::InvalidQuery),
                            },
                            match urlencoding::decode(v) {
                                Ok(i) => Value::String(i.to_string()),
                                Err(_) => return Err(HttpError::InvalidQuery),
                            },
                        );
                    }
                }
                _ => {}
            }
        }

        Ok(HttpRequest {
            page,
            method,
            addr: ip_str.to_string(),
            params: Value::Object(params),
            headers,
            data: reqdata.clone(),
        })
    }

    /// Read http request with http_rrs support
    pub fn read_with_rrs(data: &mut impl Read) -> Result<HttpRequest, HttpError> {
        let addr = match read_line_lf(data) {
            Ok(i) => i,
            Err(e) => {
                return Err(e);
            }
        }
        .to_socket_addrs()
        .unwrap()
        .collect::<Vec<SocketAddr>>()[0];
        HttpRequest::read(data, &addr)
    }

    /// Set params to query in url
    pub fn params_to_page(&mut self) {
        let mut query = String::new();

        let mut i: bool = !self.page.contains("?");

        if let Value::Object(obj) = self.params.clone() {
            for (k, v) in obj {
                query.push_str(if i { "?" } else { "&" });
                query.push_str(urlencoding::encode(k.as_str()).to_string().as_str());
                query.push_str("=");
                query.push_str(
                    urlencoding::encode(v.as_str().unwrap())
                        .to_string()
                        .as_str(),
                );
                i = false;
            }
        }

        self.page += query.as_str();
    }

    /// Set params to json data
    pub fn params_to_json(&mut self) {
        self.data = Vec::from(self.params.to_string().as_bytes());
    }

    /// Write http request to stream
    ///
    /// [`params`](Self::params) is not written to the stream, you need to use [`params_to_json`](Self::params_to_json) or [`params_to_page`](Self::params_to_page)
    pub fn write(self, data: &mut impl Write) -> Result<(), HttpError> {
        let mut head: String = String::new();
        head.push_str(&self.method);
        head.push_str(" ");
        head.push_str(&self.page);
        head.push_str(" HTTP/1.1");
        head.push_str("\r\n");

        for (k, v) in self.headers.entries() {
            head.push_str(&k);
            head.push_str(": ");
            head.push_str(&v);
            head.push_str("\r\n");
        }

        head.push_str("\r\n");

        match data.write_all(head.as_bytes()) {
            Ok(i) => i,
            Err(_) => return Err(HttpError::WriteHeadError),
        };

        if !self.data.is_empty() {
            match data.write_all(&self.data) {
                Ok(i) => i,
                Err(_) => return Err(HttpError::WriteBodyError),
            };
        }

        Ok(())
    }
}
