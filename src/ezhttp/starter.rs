use tokio::task::JoinHandle;

use super::{
    start_server_new_thread, start_server_sync,
    start_server_with_threadpool, HttpServer,
};

use std::{
    error::Error,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

/// Http server start builder
pub struct HttpServerStarter<T: HttpServer + Send + 'static> {
    http_server: T,
    support_http_rrs: bool,
    timeout: Option<Duration>,
    host: String,
    threads: usize,
}

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

impl<T: HttpServer + Send + 'static> HttpServerStarter<T> {
    /// Create new HttpServerStarter
    pub fn new(http_server: T, host: &str) -> Self {
        HttpServerStarter {
            http_server,
            support_http_rrs: false,
            timeout: None,
            host: host.to_string(),
            threads: 0,
        }
    }

    /// Set http server
    pub fn http_server(mut self, http_server: T) -> Self {
        self.http_server = http_server;
        return self;
    }

    /// Set if http_rrs is supported
    pub fn support_http_rrs(mut self, support_http_rrs: bool) -> Self {
        self.support_http_rrs = support_http_rrs;
        return self;
    }

    /// Set timeout for read & write
    pub fn timeout(mut self, timeout: Option<Duration>) -> Self {
        self.timeout = timeout;
        return self;
    }

    /// Set host
    pub fn host(mut self, host: String) -> Self {
        self.host = host;
        return self;
    }

    /// Set threads in threadpool and return builder
    ///
    /// 0 threads means that a new thread is created for each connection \
    /// 1 thread means that all connections are processed in the main thread
    pub fn threads(mut self, threads: usize) -> Self {
        self.threads = threads;
        return self;
    }

    /// Get http server
    pub fn get_http_server(self) -> T {
        self.http_server
    }

    /// Get if http_rrs is supported
    pub fn get_support_http_rrs(&self) -> bool {
        self.support_http_rrs
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
    /// 1 thread means that all connections are processed in the main thread
    pub fn get_threads(&self) -> usize {
        self.threads
    }

    /// Start http server forever with options
    pub async fn start_forever(self) -> Result<(), Box<dyn Error>> {
        let running = Arc::new(AtomicBool::new(true));

        if self.threads == 0 {
            start_server_new_thread(self.http_server, &self.host, self.timeout, self.support_http_rrs, running).await
        } else if self.threads == 1 {
            start_server_sync(self.http_server, &self.host, self.timeout, self.support_http_rrs, running).await
        } else {
            start_server_with_threadpool(
                self.http_server,
                &self.host,
                self.timeout,
                self.threads,
                self.support_http_rrs,
                running,
            ).await
        }
    }

    /// Start http server with options in new thread
    pub fn start(self) -> RunningHttpServer {
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        let thread = if self.threads == 0 {
            tokio::spawn(async move {
                start_server_new_thread(
                    self.http_server,
                    &self.host,
                    self.timeout,
                    self.support_http_rrs,
                    running_clone,
                ).await
                .expect("http server error");
            })
        } else if self.threads == 1 {
            tokio::spawn(async move {
                start_server_sync(
                    self.http_server,
                    &self.host,
                    self.timeout,
                    self.support_http_rrs,
                    running_clone,
                ).await
                .expect("http server error");
            })
        } else {
            tokio::spawn(async move {
                start_server_with_threadpool(
                    self.http_server,
                    &self.host,
                    self.timeout,
                    self.threads,
                    self.support_http_rrs,
                    running_clone,
                ).await
                .expect("http server error")
            })
        };

        RunningHttpServer::new(thread, running.clone())
    }
}
