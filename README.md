# EzHttp
Easy http server on rust

Example:
```rust

use ezhttp::{Headers, HttpResponse, HttpRequest, HttpServer};
use tokio::{runtime::Runtime, net::TcpListener};

struct EzSite {
    index_page: String
}

impl HttpServer for EzSite {
    async fn on_request(&mut self, req: &HttpRequest) -> Option<HttpResponse> {
        println!("{} > {} {}", req.addr, req.method, req.page);

        if req.page == "/" {
            Some(
                HttpResponse::from_str(
                    Headers::from(vec![
                        ("Content-Type", "text/html")
                    ]), 
                    "200 OK".to_string(), 
                    &self.index_page
                )
            )
        } else {
            None // just shutdown socket
        }
    }

    async fn on_start(&mut self, host: &str, _listener: &TcpListener) {
        println!("Http server started on {}", host);
    }

    async fn on_close(&mut self) {
        println!("Http server closed");
    }
}

impl EzSite {
    fn new(index_page: &str) -> Self {
        EzSite {
            index_page: index_page.to_string()
        }
    }
}

fn main() {
    let site = EzSite::new("Hello World!");
    let host = "localhost:8080";

    Runtime::new().unwrap().block_on(async move {
        ezhttp::start_server(site, host).await.unwrap();
    });
}

```