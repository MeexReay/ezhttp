use std::sync::atomic::{AtomicBool, Ordering};
use std::{
    boxed::Box,
    error::Error,
    future::Future,
    sync::Arc,
    time::Duration,
};

use tokio::io::AsyncReadExt;
use rusty_pool::ThreadPool;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio_io_timeout::TimeoutStream;

pub mod error;
pub mod headers;
pub mod request;
pub mod response;
pub mod starter;
pub mod handler;

pub use error::*;
pub use headers::*;
pub use request::*;
pub use response::*;
pub use starter::*;
pub use handler::*;


async fn read_line(data: &mut (impl AsyncReadExt + Unpin)) -> Result<String, HttpError> {
    let mut line = Vec::new();
    loop {
        let mut buffer = vec![0;1];
        data.read_exact(&mut buffer).await.or(Err(HttpError::ReadLineEof))?;
        let char = buffer[0];
        line.push(char);
        if char == 0x0a {
            break;
        }
    }
    String::from_utf8(line).or(Err(HttpError::ReadLineUnknown))
}

async fn read_line_crlf(data: &mut (impl AsyncReadExt + Unpin)) -> Result<String, HttpError> {
    match read_line(data).await {
        Ok(i) => Ok(i[..i.len() - 2].to_string()),
        Err(e) => Err(e),
    }
}

#[cfg(feature = "http_rrs")]
async fn read_line_lf(data: &mut (impl AsyncReadExt + Unpin)) -> Result<String, HttpError> {
    match read_line(data).await {
        Ok(i) => Ok(i[..i.len() - 1].to_string()),
        Err(e) => Err(e),
    }
}

pub type Stream = TimeoutStream<TcpStream>;

/// Async http server trait
pub trait HttpServer {
    fn on_start(&mut self, host: &str) -> impl Future<Output = ()> + Send;
    fn on_close(&mut self) -> impl Future<Output = ()> + Send;
    fn on_request(
        &mut self,
        req: &HttpRequest,
    ) -> impl Future<Output = Option<HttpResponse>> + Send;
    fn on_error(
        &mut self, 
        _: HttpError
    ) -> impl Future<Output = ()> + Send {
        async {}
    }
}

async fn start_server_with_threadpool<T>(
    server: T,
    host: &str,
    timeout: Option<Duration>,
    threads: usize,
    handler: Handler<T>,
    running: Arc<AtomicBool>,
) -> Result<(), Box<dyn Error>>
where
    T: HttpServer + Send + 'static,
{
    let threadpool = ThreadPool::new(threads, threads * 10, Duration::from_secs(60));
    let server = Arc::new(Mutex::new(server));
    let listener = TcpListener::bind(host).await?;

    let host_clone = String::from(host).clone();
    let server_clone = server.clone();
    server_clone.lock().await.on_start(&host_clone).await;

    while running.load(Ordering::Acquire) {
        let Ok((sock, _)) = listener.accept().await else { continue; };
        let mut sock = TimeoutStream::new(sock);

        sock.set_read_timeout(timeout);
        sock.set_write_timeout(timeout);

        let now_server = Arc::clone(&server);

        threadpool.spawn((&handler)(now_server, sock));
    }

    threadpool.join();

    server.lock().await.on_close().await;

    Ok(())
}

async fn start_server_new_thread<T>(
    server: T,
    host: &str,
    timeout: Option<Duration>,
    handler: Handler<T>,
    running: Arc<AtomicBool>,
) -> Result<(), Box<dyn Error>>
where
    T: HttpServer + Send + 'static,
{
    let server = Arc::new(Mutex::new(server));
    let listener = TcpListener::bind(host).await?;

    let host_clone = String::from(host).clone();
    let server_clone = server.clone();
    server_clone.lock().await.on_start(&host_clone).await;

    while running.load(Ordering::Acquire) {
        let Ok((sock, _)) = listener.accept().await else { continue; };
        let mut sock = TimeoutStream::new(sock);

        sock.set_read_timeout(timeout);
        sock.set_write_timeout(timeout);

        let now_server = Arc::clone(&server);

        tokio::spawn((&handler)(now_server, sock));
    }

    server.lock().await.on_close().await;

    Ok(())
}

async fn start_server_sync<T>(
    server: T,
    host: &str,
    timeout: Option<Duration>,
    handler: Handler<T>,
    running: Arc<AtomicBool>,
) -> Result<(), Box<dyn Error>>
where
    T: HttpServer + Send + 'static,
{
    let server = Arc::new(Mutex::new(server));
    let listener = TcpListener::bind(host).await?;

    let host_clone = String::from(host).clone();
    let server_clone = server.clone();
    server_clone.lock().await.on_start(&host_clone).await;

    while running.load(Ordering::Acquire) {
        let Ok((sock, _)) = listener.accept().await else { continue; };
        let mut sock = TimeoutStream::new(sock);

        sock.set_read_timeout(timeout);
        sock.set_write_timeout(timeout);

        let now_server = Arc::clone(&server);

        handler(now_server, sock).await;
    }

    server.lock().await.on_close().await;

    Ok(())
}

/// Start [`HttpServer`](HttpServer) on some host
///
/// Use [`HttpServerStarter`](HttpServerStarter) to set more options
pub async fn start_server<T: HttpServer + Send + 'static>(
    server: T, 
    host: &str
) -> Result<(), Box<dyn Error>> {
    start_server_new_thread(
        server,
        host,
        None,
        Box::new(move |a, b| Box::pin(handler_connection(a, b))),
        Arc::new(AtomicBool::new(true)),
    ).await
}
