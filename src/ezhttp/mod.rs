use std::{
    boxed::Box,
    error::Error,
    future::Future,
    io::Read,
    net::{TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

pub mod error;
pub mod headers;
pub mod request;
pub mod response;
pub mod starter;

pub use error::*;
pub use headers::*;
pub use request::*;
pub use response::*;
use rusty_pool::ThreadPool;
pub use starter::*;
use tokio::sync::Mutex;

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

/// Async http server trait
pub trait HttpServer {
    fn on_start(&mut self, host: &str) -> impl Future<Output = ()> + Send;
    fn on_close(&mut self) -> impl Future<Output = ()> + Send;
    fn on_request(
        &mut self,
        req: &HttpRequest,
    ) -> impl Future<Output = Option<HttpResponse>> + Send;
}

async fn start_server_with_threadpool<S>(
    server: S,
    host: &str,
    timeout: Option<Duration>,
    threads: usize,
    rrs: bool,
    running: Arc<AtomicBool>,
) -> Result<(), Box<dyn Error>>
where
    S: HttpServer + Send + 'static,
{
    let threadpool = ThreadPool::new(threads, threads * 10, Duration::from_secs(60));
    let server = Arc::new(Mutex::new(server));
    let listener = TcpListener::bind(host)?;

    let host_clone = String::from(host).clone();
    let server_clone = server.clone();
    server_clone.lock().await.on_start(&host_clone).await;

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

        if !rrs {
            threadpool.spawn(handle_connection(now_server, sock));
        } else {
            threadpool.spawn(handle_connection_rrs(now_server, sock));
        }
    }

    threadpool.join();

    server.lock().await.on_close().await;

    Ok(())
}

async fn start_server_new_thread<S>(
    server: S,
    host: &str,
    timeout: Option<Duration>,
    rrs: bool,
    running: Arc<AtomicBool>,
) -> Result<(), Box<dyn Error>>
where
    S: HttpServer + Send + 'static,
{
    let server = Arc::new(Mutex::new(server));
    let listener = TcpListener::bind(host)?;

    let host_clone = String::from(host).clone();
    let server_clone = server.clone();
    server_clone.lock().await.on_start(&host_clone).await;

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

        if !rrs {
            tokio::spawn(handle_connection(now_server, sock));
        } else {
            tokio::spawn(handle_connection_rrs(now_server, sock));
        }
    }

    server.lock().await.on_close().await;

    Ok(())
}

async fn start_server_sync<S>(
    server: S,
    host: &str,
    timeout: Option<Duration>,
    rrs: bool,
    running: Arc<AtomicBool>,
) -> Result<(), Box<dyn Error>>
where
    S: HttpServer + Send + 'static,
{
    let server = Arc::new(Mutex::new(server));
    let listener = TcpListener::bind(host)?;

    let host_clone = String::from(host).clone();
    let server_clone = server.clone();
    server_clone.lock().await.on_start(&host_clone).await;

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

        if !rrs {
            handle_connection(now_server, sock).await;
        } else {
            handle_connection_rrs(now_server, sock).await;
        }
    }

    server.lock().await.on_close().await;

    Ok(())
}

async fn handle_connection<S: HttpServer + Send + 'static>(
    server: Arc<Mutex<S>>, 
    mut sock: TcpStream
) {
    let Ok(addr) = sock.peer_addr() else { return; };

    let req = match HttpRequest::read(&mut sock, &addr) {
        Ok(i) => i,
        Err(_) => {
            return;
        }
    };

    let resp = match server.lock().await.on_request(&req).await {
        Some(i) => i,
        None => {
            return;
        }
    };

    let _ = resp.write(&mut sock);
}

async fn handle_connection_rrs<S: HttpServer + Send + 'static>(
    server: Arc<Mutex<S>>,
    mut sock: TcpStream,
) {
    let req = match HttpRequest::read_with_rrs(&mut sock) {
        Ok(i) => i,
        Err(_) => {
            return;
        }
    };
    let resp = match server.lock().await.on_request(&req).await {
        Some(i) => i,
        None => {
            return;
        }
    };
    let _ = resp.write(&mut sock);
}

/// Start [`HttpServer`](HttpServer) on some host
///
/// Use [`HttpServerStarter`](HttpServerStarter) to set more options
pub async fn start_server<S: HttpServer + Send + 'static>(
    server: S, 
    host: &str
) -> Result<(), Box<dyn Error>> {
    start_server_new_thread(
        server,
        host,
        None,
        false,
        Arc::new(AtomicBool::new(true)),
    ).await
}
