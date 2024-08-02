use super::{read_line_crlf, Headers, HttpError};

use serde_json::Value;
use std::{
    fmt::{Debug, Display},
    io::{Read, Write},
};

/// Http response
#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub headers: Headers,
    pub status_code: String,
    pub data: Vec<u8>,
}

impl Display for HttpResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self, f)
    }
}

impl HttpResponse {
    /// Create new http response with empty headers and data and a 200 OK status code
    pub fn new() -> Self {
        Self::from_bytes(Headers::new(), "200 OK", Vec::new())
    }

    /// Create new http response from headers, bytes data, and status code
    pub fn from_bytes(headers: Headers, status_code: impl ToString, data: Vec<u8>) -> Self {
        HttpResponse {
            headers,
            data,
            status_code: status_code.to_string(),
        }
    }

    /// Create new http response from headers, string data, and status code
    pub fn from_string(headers: Headers, status_code: impl ToString, data: impl ToString) -> Self {
        HttpResponse {
            headers,
            data: data.to_string().into_bytes(),
            status_code: status_code.to_string(),
        }
    }

    /// Get data in UTF-8
    pub fn get_text(self) -> String {
        match String::from_utf8(self.data) {
            Ok(i) => i,
            Err(_) => String::new(),
        }
    }

    /// Get json [`Value`](Value) from data
    pub fn get_json(self) -> Value {
        match serde_json::from_str(self.get_text().as_str()) {
            Ok(i) => i,
            Err(_) => Value::Null,
        }
    }

    /// Read http response from stream
    pub fn read(data: &mut impl Read) -> Result<HttpResponse, HttpError> {
        let status = match read_line_crlf(data) {
            Ok(i) => i,
            Err(e) => {
                return Err(e);
            }
        };

        let (_, status_code) = match status.split_once(" ") {
            Some(i) => i,
            None => return Err(HttpError::InvalidStatus),
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
        } else {
            loop {
                let mut buf: Vec<u8> = vec![0; 1024 * 32];

                let buf_len = match data.read(&mut buf) {
                    Ok(i) => i,
                    Err(_) => {
                        break;
                    }
                };

                if buf_len == 0 {
                    break;
                }

                buf.truncate(buf_len);

                reqdata.append(&mut buf);
            }
        }

        Ok(HttpResponse::from_bytes(headers, status_code, reqdata))
    }

    /// Write http response to stream
    pub fn write(self, data: &mut impl Write) -> Result<(), &str> {
        let mut head: String = String::new();
        head.push_str("HTTP/1.1 ");
        head.push_str(&self.status_code);
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
            Err(_) => return Err("write head error"),
        };

        match data.write_all(&self.data) {
            Ok(i) => i,
            Err(_) => return Err("write body error"),
        };

        Ok(())
    }
}
