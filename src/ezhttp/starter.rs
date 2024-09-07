use tokio::{runtime::Runtime, task::JoinHandle};

use super::{
    handler_connection, start_server_new_thread, start_server_sync, start_server_with_threadpool, Handler, HttpServer
};
use crate::pin_handler;

use std::{
    error::Error, sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    }, time::Duration
};

/// Running http server
pub struct RunningHttpServer {
    thread: JoinHandle<()>,
    running: Arc<AtomicBool>,
}

impl RunningHttpServer {
    fn new(thread: JoinHandle<()>, running: Arc<AtomicBool>) -> Self {
        RunningHttpServer { thread, running }
    }

    /// Stop http server
    pub fn close(&self) {
        self.running.store(false, Ordering::Release);
        self.thread.abort();
    }
}

/// Http server start builder
pub struct HttpServerStarter<T: HttpServer + Send + 'static> {
    http_server: T,
    handler: Handler<T>,
    timeout: Option<Duration>,
    host: String,
    threads: usize,
}

impl<T: HttpServer + Send + 'static + Sync> HttpServerStarter<T> {
    /// Create new HttpServerStarter
    pub fn new(http_server: T, host: &str) -> Self {
        HttpServerStarter {
            http_server,
            handler: pin_handler!(handler_connection),
            timeout: None,
            host: host.to_string(),
            threads: 0,
        }
    }

    /// Set http server
    pub fn http_server(mut self, http_server: T) -> Self {
        self.http_server = http_server;
        self
    }

    /// Set if http_rrs is supported
    pub fn handler(mut self, handler: Handler<T>) -> Self {
        self.handler = handler;
        self
    }

    /// Set timeout for read & write
    pub fn timeout(mut self, timeout: Option<Duration>) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set host
    pub fn host(mut self, host: String) -> Self {
        self.host = host;
        self
    }

    /// Set threads in threadpool and return builder
    ///
    /// 0 threads means that a new thread is created for each connection \
    /// 1 thread means that all connections are processed in the main thread
    pub fn threads(mut self, threads: usize) -> Self {
        self.threads = threads;
        self
    }

    /// Get http server
    pub fn get_http_server(&self) -> &T {
        &self.http_server
    }

    /// Get if http_rrs is supported
    pub fn get_handler(&self) -> &Handler<T> {
        &self.handler
    }

    /// Get timeout for read & write
    pub fn get_timeout(&self) -> Option<Duration> {
        self.timeout
    }

    /// Get host
    pub fn get_host(&self) -> &str {
        &self.host
    }

    /// Get threads in threadpool
    ///
    /// 0 threads means that a new thread is created for each connection \
    /// 1 thread means that all connections are processed in the one thread
    pub fn get_threads(&self) -> usize {
        self.threads
    }

    /// Start http server forever with options
    pub async fn start_forever(self) -> Result<(), Box<dyn Error>> {
        let running = Arc::new(AtomicBool::new(true));

        if self.threads == 0 {
            start_server_new_thread(self.http_server, &self.host, self.timeout, self.handler, running).await
        } else if self.threads == 1 {
            start_server_sync(self.http_server, &self.host, self.timeout, self.handler, running).await
        } else {
            start_server_with_threadpool(
                self.http_server,
                &self.host,
                self.timeout,
                self.threads,
                self.handler,
                running,
            ).await
        }
    }

    /// Start http server with options in new thread
    pub fn start(self) -> RunningHttpServer {
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        let thread = if self.threads == 0 {
            Runtime::new().unwrap().spawn(async move {
                start_server_new_thread(
                    self.http_server,
                    &self.host,
                    self.timeout,
                    self.handler,
                    running_clone,
                ).await
                .expect("http server error");
            })
        } else if self.threads == 1 {
            Runtime::new().unwrap().spawn(async move {
                start_server_sync(
                    self.http_server,
                    &self.host,
                    self.timeout,
                    self.handler,
                    running_clone,
                ).await
                .expect("http server error");
            })
        } else {
            Runtime::new().unwrap().spawn(async move {
                start_server_with_threadpool(
                    self.http_server,
                    &self.host,
                    self.timeout,
                    self.threads,
                    self.handler,
                    running_clone,
                ).await
                .expect("http server error")
            })
        };

        RunningHttpServer::new(thread, running.clone())
    }
}
