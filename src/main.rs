use ezhttp::{Headers, HttpRequest, HttpResponse, HttpServer, HttpServerStarter};
use std::time::Duration;

struct EzSite {
    index_page: String,
}

impl EzSite {
    fn new(index_page: &str) -> Self {
        EzSite {
            index_page: index_page.to_string(),
        }
    }
}

impl HttpServer for EzSite {
    async fn on_request(&mut self, req: &HttpRequest) -> Option<HttpResponse> {
        println!("{} > {} {}", req.addr, req.method, req.page);

        if req.page == "/" {
            Some(HttpResponse::from_string(
                Headers::from(vec![("Content-Type", "text/html")]), // response headers
                "200 OK",                              // response status code
                self.index_page.clone(),                      // response body
            ))
        } else {
            None // close connection
        }
    }

    async fn on_start(&mut self, host: &str) {
        println!("Http server started on {}", host);
    }

    async fn on_close(&mut self) {
        println!("Http server closed");
    }
}

fn main() {
    let site = EzSite::new("Hello World!");
    let host = "localhost:8080";

    HttpServerStarter::new(site, host)
        .timeout(Some(Duration::from_secs(5))) // read & write timeout
        .threads(5) // threadpool size
        .start_forever()
        .expect("http server error");

    // ezhttp::start_server(site, host);
}
