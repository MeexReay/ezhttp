use crate::Sendable;

use super::{
    HttpServer, 
    super::{
        Stream,
        request::HttpRequest
    }
};

use std::{future::Future, pin::Pin, sync::Arc};
use tokio::net::TcpStream;
use tokio_io_timeout::TimeoutStream;

pub type Handler<T> = Box<dyn Fn(Arc<T>, TimeoutStream<TcpStream>) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

/// Default connection handler
/// Turns input to request and response to output
pub async fn handler_connection<S: HttpServer + Send + 'static + Sync>(
    server: Arc<S>, 
    mut sock: Stream
) {
    let Ok(addr) = sock.get_ref().peer_addr() else { return; };

    loop {
        let req = match HttpRequest::recv(sock.get_mut(), &addr).await {
            Ok(i) => i,
            Err(e) => {
                server.on_error(e).await;
                return;
            }
        };

        let resp = match server.on_request(&req).await {
            Some(i) => i,
            None => {
                return;
            }
        };

        match resp.send(sock.get_mut()).await {
            Ok(_) => {},
            Err(e) => {
                server.on_error(e).await;
                return;
            },
        }
    }
}

#[macro_export]
macro_rules! pin_handler {
    ($handler: expr) => {
        Box::new(move |a, b| Box::pin($handler(a, b)))
    };
}