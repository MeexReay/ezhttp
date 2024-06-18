use ezhttp::{Headers, HttpRequest, HttpResponse, HttpServer};

struct EzSite {
    index_page: String,
}

impl HttpServer for EzSite {
    async fn on_request(&mut self, req: &HttpRequest) -> Option<HttpResponse> {
        println!("{} > {} {}", req.addr, req.method, req.page);

        if req.page == "/" {
            Some(HttpResponse::from_str(
                Headers::from(vec![("Content-Type", "text/html")]), // response headers
                "200 OK".to_string(),                               // response status code
                &self.index_page,                                   // response body
            ))
        } else {
            None // shutdown request
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
    let site = EzSite::new(&"Hello World!".repeat(99999));
    let host = "localhost:8080";

    ezhttp::start_server(site, host).unwrap();

    // ezhttp::start_server_timeout(site, host, Duration::from_secs(5)).unwrap(); // with read and write timeout
}
