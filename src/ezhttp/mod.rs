use futures::executor::block_on;
use std::{
    boxed::Box,
    error::Error,
    future::Future,
    io::Read,
    net::{TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};
use threadpool::ThreadPool;

pub mod error;
pub mod headers;
pub mod request;
pub mod response;
pub mod starter;

pub use error::*;
pub use headers::*;
pub use request::*;
pub use response::*;
pub use starter::*;

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

/// Start [`HttpServer`](HttpServer) on some host
///
/// Use [`HttpServerStarter`](HttpServerStarter) to set more options
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
