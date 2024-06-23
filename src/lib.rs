use futures::executor::block_on;
use serde_json::Value;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use std::{
    boxed::Box,
    error::Error,
    fmt::{Debug, Display},
    future::Future,
    net::{IpAddr, SocketAddr, ToSocketAddrs},
    sync::Arc,
    thread,
};
use std::{
    net::{TcpListener, TcpStream},
    sync::{mpsc, Mutex},
};

#[derive(Clone, Debug)]
pub struct Headers {
    entries: Vec<(String, String)>,
}

impl Into<HashMap<String, String>> for Headers {
    fn into(self) -> HashMap<String, String> {
        HashMap::from_iter(self.entries().into_iter())
    }
}

impl<T, U> From<Vec<(T, U)>> for Headers
where
    T: ToString,
    U: ToString,
{
    fn from(value: Vec<(T, U)>) -> Self {
        Headers {
            entries: value
                .into_iter()
                .map(|v| (v.0.to_string(), v.1.to_string()))
                .collect(),
        }
    }
}

impl Headers {
    pub fn new() -> Self {
        Headers {
            entries: Vec::new(),
        }
    }

    pub fn contains_value(self, value: impl ToString) -> bool {
        for (_, v) in self.entries {
            if v == value.to_string() {
                return true;
            }
        }
        return false;
    }

    pub fn contains_key(self, key: impl ToString) -> bool {
        for (k, _) in self.entries {
            if k.to_lowercase() == key.to_string().to_lowercase() {
                return true;
            }
        }
        return false;
    }

    pub fn get(self, key: impl ToString) -> Option<String> {
        for (k, v) in self.entries {
            if k.to_lowercase() == key.to_string().to_lowercase() {
                return Some(v);
            }
        }
        return None;
    }

    pub fn put(&mut self, key: impl ToString, value: String) {
        for t in self.entries.iter_mut() {
            if t.0.to_lowercase() == key.to_string().to_lowercase() {
                t.1 = value;
                return;
            }
        }
        self.entries.push((key.to_string(), value));
    }

    pub fn remove(&mut self, key: impl ToString) {
        for (i, t) in self.entries.iter_mut().enumerate() {
            if t.0.to_lowercase() == key.to_string().to_lowercase() {
                self.entries.remove(i);
                return;
            }
        }
    }

    pub fn keys(self) -> Vec<String> {
        self.entries.iter().map(|e| e.0.clone()).collect()
    }

    pub fn values(self) -> Vec<String> {
        self.entries.iter().map(|e| e.1.clone()).collect()
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

impl Display for Headers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self, f)
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

impl Display for HttpRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self, f)
    }
}

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

fn read_line(data: &mut impl Read) -> Result<String, HttpError> {
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

fn read_line_crlf(data: &mut impl Read) -> Result<String, HttpError> {
    match read_line(data) {
        Ok(i) => Ok(i[..i.len() - 2].to_string()),
        Err(e) => Err(e),
    }
}

fn read_line_lf(data: &mut impl Read) -> Result<String, HttpError> {
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
            page: page,
            method: method,
            addr: ip_str.to_string(),
            params: Value::Object(params),
            headers: headers,
            data: reqdata.clone(),
        })
    }

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

    pub fn write(self, data: &mut impl Write) -> Result<(), HttpError> {
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
    pub fn new() -> Self {
        Self::from_bytes(Headers::new(), "200 OK", Vec::new())
    }

    pub fn from_bytes(headers: Headers, status_code: impl ToString, data: Vec<u8>) -> Self {
        HttpResponse {
            headers: headers,
            data: data,
            status_code: status_code.to_string(),
        }
    }

    pub fn from_string(headers: Headers, status_code: impl ToString, data: impl ToString) -> Self {
        HttpResponse {
            headers: headers,
            data: data.to_string().into_bytes(),
            status_code: status_code.to_string(),
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

        Ok(HttpResponse {
            headers: headers,
            status_code: status_code.to_string(),
            data: reqdata,
        })
    }

    pub fn write(self, data: &mut impl Write) -> Result<(), &str> {
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

pub trait HttpServer {
    fn on_start(&mut self, host: &str) -> impl Future<Output = ()> + Send;
    fn on_close(&mut self) -> impl Future<Output = ()> + Send;
    fn on_request(
        &mut self,
        req: &HttpRequest,
    ) -> impl Future<Output = Option<HttpResponse>> + Send;
}

pub struct HttpServerStarter<T: HttpServer + Send + 'static> {
    http_server: T,
    support_http_rrs: bool,
    timeout: Option<Duration>,
    host: String,
    threads: usize,
}

pub struct RunningHttpServer {
    thread: thread::JoinHandle<()>,
    running: Arc<AtomicBool>,
}

impl RunningHttpServer {
    fn new(thread: thread::JoinHandle<()>, running: Arc<AtomicBool>) -> Self {
        RunningHttpServer { thread, running }
    }

    pub fn close(self) {
        self.running.store(false, Ordering::Release);
        self.thread.join().unwrap();
    }
}

impl<T: HttpServer + Send + 'static> HttpServerStarter<T> {
    pub fn new(http_server: T, host: &str) -> Self {
        HttpServerStarter {
            http_server,
            support_http_rrs: false,
            timeout: None,
            host: host.to_string(),
            threads: 0,
        }
    }

    pub fn http_server(mut self, http_server: T) -> Self {
        self.http_server = http_server;
        return self;
    }

    pub fn support_http_rrs(mut self, support_http_rrs: bool) -> Self {
        self.support_http_rrs = support_http_rrs;
        return self;
    }

    pub fn timeout(mut self, timeout: Option<Duration>) -> Self {
        self.timeout = timeout;
        return self;
    }

    pub fn host(mut self, host: String) -> Self {
        self.host = host;
        return self;
    }

    pub fn threads(mut self, threads: usize) -> Self {
        self.threads = threads;
        return self;
    }

    pub fn start_forever(self) -> Result<(), Box<dyn Error>> {
        let handler = if self.support_http_rrs {
            move |server, sock| {
                handle_connection_rrs(server, sock);
            }
        } else {
            move |server, sock| {
                handle_connection(server, sock);
            }
        };

        let running = Arc::new(AtomicBool::new(true));

        if self.threads == 0 {
            start_server_new_thread(self.http_server, &self.host, self.timeout, handler, running)
        } else if self.threads == 1 {
            start_server_sync(self.http_server, &self.host, self.timeout, handler, running)
        } else {
            start_server_with_threadpool(
                self.http_server,
                &self.host,
                self.timeout,
                self.threads,
                handler,
                running,
            )
        }
    }

    pub fn start(self) -> RunningHttpServer {
        let handler = if self.support_http_rrs {
            move |server, sock| {
                handle_connection_rrs(server, sock);
            }
        } else {
            move |server, sock| {
                handle_connection(server, sock);
            }
        };

        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        let thread = if self.threads == 0 {
            thread::spawn(move || {
                start_server_new_thread(
                    self.http_server,
                    &self.host,
                    self.timeout,
                    handler,
                    running_clone,
                )
                .expect("http server error");
            })
        } else if self.threads == 1 {
            thread::spawn(move || {
                start_server_sync(
                    self.http_server,
                    &self.host,
                    self.timeout,
                    handler,
                    running_clone,
                )
                .expect("http server error");
            })
        } else {
            thread::spawn(move || {
                start_server_with_threadpool(
                    self.http_server,
                    &self.host,
                    self.timeout,
                    self.threads,
                    handler,
                    running_clone,
                )
                .expect("http server error")
            })
        };

        RunningHttpServer::new(thread, running.clone())
    }
}

fn start_server_with_threadpool<F, S>(
    server: S,
    host: &str,
    timeout: Option<Duration>,
    threads: usize,
    handler: F,
    running: Arc<AtomicBool>,
) -> Result<(), Box<dyn Error>>
where
    F: (Fn(Arc<Mutex<S>>, TcpStream) -> ()) + Send + 'static + Copy,
    S: HttpServer + Send + 'static,
{
    let threadpool = ThreadPool::new(threads);
    let server = Arc::new(Mutex::new(server));
    let listener = TcpListener::bind(host)?;

    let host_clone = String::from(host).clone();
    let server_clone = server.clone();
    block_on(server_clone.lock().unwrap().on_start(&host_clone));

    listener.set_nonblocking(true)?;

    while running.load(Ordering::Acquire) {
        let (sock, _) = match listener.accept() {
            Ok(i) => i,
            Err(_) => {
                continue;
            }
        };

        sock.set_read_timeout(timeout).unwrap();
        sock.set_write_timeout(timeout).unwrap();

        let now_server = Arc::clone(&server);
        threadpool.execute(move || {
            handler(now_server, sock);
        });
    }

    threadpool.join();

    block_on(server.lock().unwrap().on_close());

    Ok(())
}

fn start_server_new_thread<F, S>(
    server: S,
    host: &str,
    timeout: Option<Duration>,
    handler: F,
    running: Arc<AtomicBool>,
) -> Result<(), Box<dyn Error>>
where
    F: (Fn(Arc<Mutex<S>>, TcpStream) -> ()) + Send + 'static + Copy,
    S: HttpServer + Send + 'static,
{
    let server = Arc::new(Mutex::new(server));
    let listener = TcpListener::bind(host)?;

    let host_clone = String::from(host).clone();
    let server_clone = server.clone();
    block_on(server_clone.lock().unwrap().on_start(&host_clone));

    while running.load(Ordering::Acquire) {
        let (sock, _) = match listener.accept() {
            Ok(i) => i,
            Err(_) => {
                continue;
            }
        };

        sock.set_read_timeout(timeout).unwrap();
        sock.set_write_timeout(timeout).unwrap();

        let now_server = Arc::clone(&server);
        thread::spawn(move || {
            handler(now_server, sock);
        });
    }

    block_on(server.lock().unwrap().on_close());

    Ok(())
}

fn start_server_sync<F, S>(
    server: S,
    host: &str,
    timeout: Option<Duration>,
    handler: F,
    running: Arc<AtomicBool>,
) -> Result<(), Box<dyn Error>>
where
    F: (Fn(Arc<Mutex<S>>, TcpStream) -> ()) + Send + 'static + Copy,
    S: HttpServer + Send + 'static,
{
    let server = Arc::new(Mutex::new(server));
    let listener = TcpListener::bind(host)?;

    let host_clone = String::from(host).clone();
    let server_clone = server.clone();
    block_on(server_clone.lock().unwrap().on_start(&host_clone));

    while running.load(Ordering::Acquire) {
        let (sock, _) = match listener.accept() {
            Ok(i) => i,
            Err(_) => {
                continue;
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

type Job = Box<dyn FnOnce() + Send + 'static>;

struct ThreadPool {
    workers: Vec<Worker>,
    sender: mpsc::Sender<Option<Job>>,
}

impl ThreadPool {
    fn new(size: usize) -> ThreadPool {
        assert!(size > 0);

        let (sender, receiver) = mpsc::channel();
        let receiver = Arc::new(Mutex::new(receiver));

        let mut workers = Vec::with_capacity(size);

        for _ in 0..size {
            workers.push(Worker::new(Arc::clone(&receiver)));
        }

        ThreadPool { workers, sender }
    }

    fn join(self) {
        for _ in 0..self.workers.len() {
            self.sender.send(None).unwrap();
        }

        for ele in self.workers.into_iter() {
            ele.thread.join().unwrap();
        }
    }

    fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);

        self.sender.send(Some(job)).unwrap();
    }
}

struct Worker {
    thread: thread::JoinHandle<()>,
}

impl Worker {
    fn new(receiver: Arc<Mutex<mpsc::Receiver<Option<Job>>>>) -> Worker {
        let thread = thread::spawn(move || {
            while let Ok(Some(job)) = receiver.lock().unwrap().recv() {
                job();
            }
        });

        Worker { thread }
    }
}

pub fn start_server<S: HttpServer + Send + 'static>(server: S, host: &str) {
    start_server_new_thread(
        server,
        host,
        None,
        handle_connection,
        Arc::new(AtomicBool::new(true)),
    )
    .unwrap();
}
