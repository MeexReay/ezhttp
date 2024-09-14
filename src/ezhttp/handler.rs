use super::{HttpRequest, HttpServer, Stream};

use std::{future::Future, pin::Pin, sync::Arc};
use tokio::net::TcpStream;
use tokio_io_timeout::TimeoutStream;

#[cfg(feature = "flowgate")]
use {super::read_line_lf, std::net::{ToSocketAddrs, SocketAddr}};

pub type Handler<T> = Box<dyn Fn(Arc<T>, TimeoutStream<TcpStream>) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

/// Default connection handler
/// Turns input to request and response to output
pub async fn handler_connection<S: HttpServer + Send + 'static + Sync>(
    server: Arc<S>, 
    mut sock: Stream
) {
    let Ok(addr) = sock.get_ref().peer_addr() else { return; };

    let req = match HttpRequest::read(sock.get_mut(), &addr).await {
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

    match resp.write(sock.get_mut()).await {
        Ok(_) => {},
        Err(e) => {
            server.on_error(e).await;
            return;
        },
    }

}

#[macro_export]
macro_rules! pin_handler {
    ($handler: expr) => {
        Box::new(move |a, b| Box::pin($handler(a, b)))
    };
}

#[cfg(feature = "flowgate")]
/// Flowgate handler
pub async fn handler_flowgate<S: HttpServer + Send + 'static + Sync>(
    server: Arc<S>,
    mut sock: Stream,
) {
    let addr = match read_line_lf(sock.get_mut()).await {
        Ok(i) => i,
        Err(e) => {
            server.on_error(e).await;
            return;
        }
    }
    .to_socket_addrs()
    .unwrap()
    .collect::<Vec<SocketAddr>>()[0];

    let req = match HttpRequest::read(sock.get_mut(), &addr).await {
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

    match resp.write(sock.get_mut()).await {
        Ok(_) => {},
        Err(e) => {
            server.on_error(e).await;
            return;
        },
    }
}