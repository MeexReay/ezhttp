# EzHttp
Easy http server on rust

This library is under developement, so if you found any bugs, please write them to [Issues](https://github.com/MeexReay/ezhttp/issues)

Example:
```rust
use ezhttp::{Headers, HttpRequest, HttpResponse, HttpServer};

struct EzSite {
    index_page: String,
}

impl HttpServer for EzSite {
    async fn on_request(&mut self, req: &HttpRequest) -> Option<HttpResponse> {
        println!("{} > {} {}", req.addr, req.method, req.page);

        if req.page == "/" {
            Some(HttpResponse::from_str(
                Headers::from(vec![("Content-Type", "text/html")]),
                "200 OK".to_string(),
                &self.index_page,
            ))
        } else {
            None // just shutdown socket
        }
    }

    async fn on_start(&mut self, host: &str) {
        println!("Http server started on {}", host);
    }

    async fn on_close(&mut self) {
        println!("Http server closed");
    }
}

impl EzSite {
    fn new(index_page: &str) -> Self {
        EzSite {
            index_page: index_page.to_string(),
        }
    }
}

fn main() {
    let site = EzSite::new("Hello World!");
    let host = "localhost:8080";

    ezhttp::start_server(site, host).unwrap();
}

```
