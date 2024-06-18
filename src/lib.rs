use futures::executor::block_on;
use serde_json::Value;
use std::io::{Read, Write};
use std::time::Duration;
use std::{
    boxed::Box,
    error::Error,
    future::Future,
    net::{IpAddr, SocketAddr, ToSocketAddrs},
    sync::Arc,
    thread,
};
use std::{
    net::{TcpListener, TcpStream},
    sync::Mutex,
};

#[derive(Clone, Debug)]
pub struct Headers {
    entries: Vec<(String, String)>,
}

impl Headers {
    pub fn from_entries(entries: Vec<(String, String)>) -> Self {
        Headers { entries }
    }

    pub fn from(entries: Vec<(&str, &str)>) -> Self {
        Headers {
            entries: entries
                .iter()
                .map(|v| (v.0.to_string(), v.1.to_string()))
                .collect(),
        }
    }

    pub fn new() -> Self {
        Headers {
            entries: Vec::new(),
        }
    }

    pub fn contains_value(self, value: String) -> bool {
        for (_, v) in self.entries {
            if v == value {
                return true;
            }
        }
        return false;
    }

    pub fn contains_key(self, key: String) -> bool {
        for (k, _) in self.entries {
            if k == key.to_lowercase() {
                return true;
            }
        }
        return false;
    }

    pub fn get(self, key: String) -> Option<String> {
        for (k, v) in self.entries {
            if k == key.to_lowercase() {
                return Some(v);
            }
        }
        return None;
    }

    pub fn put(&mut self, key: String, value: String) {
        for t in self.entries.iter_mut() {
            if t.0 == key.to_lowercase() {
                t.1 = value;
                return;
            }
        }
        self.entries.push((key.to_lowercase(), value));
    }

    pub fn remove(&mut self, key: String) {
        for (i, t) in self.entries.iter_mut().enumerate() {
            if t.0 == key.to_lowercase() {
                self.entries.remove(i);
                return;
            }
        }
    }

    pub fn keys(self) -> Vec<String> {
        let mut keys = Vec::new();
        for (k, _) in self.entries {
            keys.push(k.to_lowercase());
        }
        keys
    }

    pub fn values(self) -> Vec<String> {
        let mut values = Vec::new();
        for (_, v) in self.entries {
            values.push(v);
        }
        values
    }

    pub fn entries(self) -> Vec<(String, String)> {
        return self.entries;
    }

    pub fn len(self) -> usize {
        return self.entries.len();
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl std::fmt::Display for Headers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub page: String,
    pub method: String,
    pub addr: String,
    pub headers: Headers,
    pub params: Value,
    pub data: Vec<u8>,
}

impl std::fmt::Display for HttpRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub headers: Headers,
    pub status_code: String,
    pub data: Vec<u8>,
}

impl std::fmt::Display for HttpResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

#[derive(Debug)]
pub enum HttpError {
    ReadLineEof,
    ReadLineUnknown,
    InvalidHeaders,
    InvalidQuery,
    InvalidContentSize,
    InvalidContent,
    JsonParseError,
    WriteHeadError,
    WriteBodyError,
    InvalidStatus,
}

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

impl Error for HttpError {}

fn read_line(data: &mut TcpStream) -> Result<String, HttpError> {
    let mut bytes = Vec::new();

    for byte in data.bytes() {
        let byte = match byte {
            Ok(i) => i,
            Err(_) => return Err(HttpError::ReadLineEof),
        };

        bytes.push(byte);

        if byte == 0x0A {
            break;
        }
    }

    match String::from_utf8(bytes) {
        Ok(i) => Ok(i),
        Err(_) => Err(HttpError::ReadLineUnknown),
    }
}

fn read_line_crlf(data: &mut TcpStream) -> Result<String, HttpError> {
    match read_line(data) {
        Ok(i) => Ok(i[..i.len() - 2].to_string()),
        Err(e) => Err(e),
    }
}

fn read_line_lf(data: &mut TcpStream) -> Result<String, HttpError> {
    match read_line(data) {
        Ok(i) => Ok(i[..i.len() - 1].to_string()),
        Err(e) => Err(e),
    }
}

fn rem_first(value: &str) -> &str {
    let mut chars = value.chars();
    chars.next();
    chars.as_str()
}

fn split(text: String, delimiter: &str, times: usize) -> Vec<String> {
    match times {
        0 => text.split(delimiter).map(|v| v.to_string()).collect(),
        1 => {
            let mut v: Vec<String> = Vec::new();
            match text.split_once(delimiter) {
                Some(i) => {
                    v.push(i.0.to_string());
                    v.push(i.1.to_string());
                }
                None => {
                    v.push(text);
                }
            }
            v
        }
        _ => text
            .splitn(times, delimiter)
            .map(|v| v.to_string())
            .collect(),
    }
}

impl HttpRequest {
    pub fn new(page: &str, method: &str, params: Value, headers: Headers, data: Vec<u8>) -> Self {
        HttpRequest {
            page: page.to_string(),
            method: method.to_string(),
            addr: String::new(),
            params: params,
            headers: headers,
            data: data,
        }
    }

    pub fn read(data: &mut TcpStream, addr: &SocketAddr) -> Result<HttpRequest, HttpError> {
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
            page: page,
            method: method,
            addr: ip_str.to_string(),
            params: Value::Object(params),
            headers: headers,
            data: reqdata.clone(),
        })
    }

    pub fn read_with_rrs(data: &mut TcpStream) -> Result<HttpRequest, HttpError> {
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

    pub fn write(self, data: &mut TcpStream) -> Result<(), HttpError> {
        let mut head: String = String::new();
        head.push_str(&self.method);
        head.push_str(" ");
        head.push_str(&self.page);
        head.push_str(" HTTP/1.1");
        head.push_str("\r\n");

        for (k, v) in self.headers.entries {
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

impl HttpResponse {
    pub fn new(headers: Headers, status_code: String, data: Vec<u8>) -> Self {
        HttpResponse {
            headers: headers,
            data: data,
            status_code: status_code,
        }
    }

    pub fn from_str(headers: Headers, status_code: String, data: &str) -> Self {
        HttpResponse {
            headers: headers,
            data: data.to_string().into_bytes(),
            status_code: status_code,
        }
    }

    pub fn from_string(headers: Headers, status_code: String, data: String) -> Self {
        HttpResponse {
            headers: headers,
            data: data.into_bytes(),
            status_code: status_code,
        }
    }

    pub fn get_text(self) -> String {
        match String::from_utf8(self.data) {
            Ok(i) => i,
            Err(_) => String::new(),
        }
    }

    pub fn get_json(self) -> Value {
        match serde_json::from_str(self.get_text().as_str()) {
            Ok(i) => i,
            Err(_) => Value::Null,
        }
    }

    pub fn read(data: &mut TcpStream) -> Result<HttpResponse, HttpError> {
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

        Ok(HttpResponse {
            headers: headers,
            status_code: status_code.to_string(),
            data: reqdata,
        })
    }

    pub fn write(self, data: &mut TcpStream) -> Result<(), &str> {
        let mut head: String = String::new();
        head.push_str("HTTP/1.1 ");
        head.push_str(&self.status_code);
        head.push_str("\r\n");

        for (k, v) in self.headers.entries {
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

pub trait HttpServer: Sync {
    fn on_start(&mut self, host: &str) -> impl Future<Output = ()> + Send;
    fn on_close(&mut self) -> impl Future<Output = ()> + Send;
    fn on_request(
        &mut self,
        req: &HttpRequest,
    ) -> impl Future<Output = Option<HttpResponse>> + Send;
}

fn start_server_with_handler<F, S>(
    server: S,
    host: &str,
    timeout: Option<Duration>,
    handler: F,
) -> Result<(), Box<dyn Error>>
where
    F: Fn(Arc<Mutex<S>>, TcpStream) -> (),
    S: HttpServer + Send + 'static,
    F: Send + 'static,
{
    let server = Arc::new(Mutex::new(server));
    let listener = TcpListener::bind(host)?;

    let host_clone = String::from(host).clone();
    let server_clone = server.clone();
    block_on(server_clone.lock().unwrap().on_start(&host_clone));

    loop {
        let (sock, _) = match listener.accept() {
            Ok(i) => i,
            Err(_) => {
                break;
            }
        };

        sock.set_read_timeout(timeout).unwrap();
        sock.set_write_timeout(timeout).unwrap();

        let now_server = Arc::clone(&server);
        handler(now_server, sock);
    }

    block_on(server.lock().unwrap().on_close());

    Ok(())
}

pub fn start_server(
    server: impl HttpServer + Send + 'static,
    host: &str,
) -> Result<(), Box<dyn Error>> {
    start_server_with_handler(server, host, None, move |server, sock| {
        thread::spawn(move || handle_connection(server, sock));
    })
}

// http rrs support
pub fn start_server_rrs(
    server: impl HttpServer + Send + 'static,
    host: &str,
) -> Result<(), Box<dyn Error>> {
    start_server_with_handler(server, host, None, move |server, sock| {
        thread::spawn(move || handle_connection_rrs(server, sock));
    })
}

pub fn start_server_timeout(
    server: impl HttpServer + Send + 'static,
    host: &str,
    timeout: Duration,
) -> Result<(), Box<dyn Error>> {
    start_server_with_handler(server, host, Some(timeout), move |server, sock| {
        thread::spawn(move || handle_connection(server, sock));
    })
}

// http rrs support
pub fn start_server_rrs_timeout(
    server: impl HttpServer + Send + 'static,
    host: &str,
    timeout: Duration,
) -> Result<(), Box<dyn Error>> {
    start_server_with_handler(server, host, Some(timeout), move |server, sock| {
        thread::spawn(move || handle_connection_rrs(server, sock));
    })
}

fn handle_connection<S: HttpServer + Send + 'static>(server: Arc<Mutex<S>>, mut sock: TcpStream) {
    let addr = sock.peer_addr().unwrap();

    let req = match HttpRequest::read(&mut sock, &addr) {
        Ok(i) => i,
        Err(_) => {
            return;
        }
    };
    let resp = match block_on(server.lock().unwrap().on_request(&req)) {
        Some(i) => i,
        None => {
            return;
        }
    };
    resp.write(&mut sock).unwrap();
}

// http rrs support
fn handle_connection_rrs<S: HttpServer + Send + 'static>(
    server: Arc<Mutex<S>>,
    mut sock: TcpStream,
) {
    let req = match HttpRequest::read_with_rrs(&mut sock) {
        Ok(i) => i,
        Err(_) => {
            return;
        }
    };
    let resp = match block_on(server.lock().unwrap().on_request(&req)) {
        Some(i) => i,
        None => {
            return;
        }
    };
    resp.write(&mut sock).unwrap();
}
