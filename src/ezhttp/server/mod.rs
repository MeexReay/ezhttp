use std::sync::atomic::{AtomicBool, Ordering};
use std::{
    boxed::Box,
    error::Error,
    sync::Arc,
    time::Duration,
};

use async_trait::async_trait;
use threadpool::ThreadPool;
use tokio::net::TcpListener;
use tokio::runtime::Handle;
use tokio_io_timeout::TimeoutStream;

use crate::pin_handler;

use super::error::HttpError;
use super::request::HttpRequest;
use super::Sendable;

pub mod handler;
pub mod starter;

use handler::{handler_connection, Handler};

/// Async http server trait
#[async_trait]
pub trait HttpServer {
    async fn on_start(&self, host: &str) -> ();
    async fn on_close(&self) -> ();
    async fn on_request(
        &self,
        req: &HttpRequest,
    ) -> Option<Box<dyn Sendable>>;
    async fn on_error(
        &self, 
        _: HttpError
    ) -> () {}
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
    T: HttpServer + Send + 'static + Sync,
{
    let threadpool = ThreadPool::new(threads);

    let server = Arc::new(server);
    let listener = TcpListener::bind(host).await?;
    let old_handler = handler;
    let handler = Arc::new(move |now_server, sock| { 
        Handle::current().block_on(old_handler(now_server, sock)); 
    });

    let host_clone = String::from(host).clone();
    let server_clone = server.clone();
    server_clone.on_start(&host_clone).await;

    while running.load(Ordering::Acquire) {
        let Ok((sock, _)) = listener.accept().await else { continue; };
        let mut sock = TimeoutStream::new(sock);

        sock.set_read_timeout(timeout);
        sock.set_write_timeout(timeout);

        let now_server = Arc::clone(&server);
        let now_handler = Arc::clone(&handler);

        threadpool.execute(move || {
            (now_handler)(now_server, sock);
        });
    }

    threadpool.join();

    server.on_close().await;

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
    let server = Arc::new(server);
    let listener = TcpListener::bind(host).await?;

    let host_clone = String::from(host).clone();
    let server_clone = server.clone();
    server_clone.on_start(&host_clone).await;

    while running.load(Ordering::Acquire) {
        let Ok((sock, _)) = listener.accept().await else { continue; };
        let mut sock = TimeoutStream::new(sock);

        sock.set_read_timeout(timeout);
        sock.set_write_timeout(timeout);

        let now_server = Arc::clone(&server);

        tokio::spawn((&handler)(now_server, sock));
    }

    server.on_close().await;

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
    let server = Arc::new(server);
    let listener = TcpListener::bind(host).await?;

    let host_clone = String::from(host).clone();
    let server_clone = server.clone();
    server_clone.on_start(&host_clone).await;

    while running.load(Ordering::Acquire) {
        let Ok((sock, _)) = listener.accept().await else { continue; };
        let mut sock = TimeoutStream::new(sock);

        sock.set_read_timeout(timeout);
        sock.set_write_timeout(timeout);

        let now_server = Arc::clone(&server);

        handler(now_server, sock).await;
    }

    server.on_close().await;

    Ok(())
}

/// Start [`HttpServer`](HttpServer) on some host
///
/// Use [`HttpServerStarter`](HttpServerStarter) to set more options
pub async fn start_server<T: HttpServer + Send + 'static + Sync>(
    server: T, 
    host: &str
) -> Result<(), Box<dyn Error>> {
    start_server_new_thread(
        server,
        host,
        None,
        pin_handler!(handler_connection),
        Arc::new(AtomicBool::new(true)),
    ).await
}