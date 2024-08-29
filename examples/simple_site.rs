use ezhttp::{Headers, HttpRequest, HttpResponse, HttpServer, HttpServerStarter};
use std::time::Duration;

struct EzSite {
    main_page: String,
}

impl EzSite {
    fn new(index_page: &str) -> Self {
        EzSite {
            main_page: index_page.to_string(),
        }
    }

    fn ok_response(&self, content: String) -> HttpResponse {
        HttpResponse::from_string(
            Headers::from(vec![("Content-Type", "text/html")]),
            "200 OK".to_string(),
            content,
        )
    }

    fn not_found_response(&self, content: String) -> HttpResponse {
        HttpResponse::from_string(
            Headers::from(vec![("Content-Type", "text/html")]),
            "404 Not Found".to_string(),
            content,
        )
    }

    async fn get_main_page(&self, req: &HttpRequest) -> Option<HttpResponse> {
        if req.page == "/" {
            Some(self.ok_response(self.main_page.clone()))
        } else {
            None
        }
    }

    async fn get_unknown_page(&self, req: &HttpRequest) -> Option<HttpResponse> {
        Some(self.not_found_response(format!("<h1>404 Error</h1>Not Found {}", &req.page)))
    }
}

impl HttpServer for EzSite {
    async fn on_request(&self, req: &HttpRequest) -> Option<HttpResponse> {
        println!("{} > {} {}", req.addr, req.method, req.page);

        if let Some(resp) = self.get_main_page(req).await {
            Some(resp)
        } else if let Some(resp) = self.get_unknown_page(req).await {
            Some(resp)
        } else {
            None // shutdown socket
        }
    }

    async fn on_start(&self, host: &str) {
        println!("Http server started on {}", host);
    }

    async fn on_close(&self) {
        println!("Http server closed");
    }
}

#[tokio::main]
async fn main() {
    let site = EzSite::new("<h1>Hello World!</h1>");
    let host = "localhost:8080";

    HttpServerStarter::new(site, host)
        .timeout(Some(Duration::from_secs(5)))
        .threads(5)
        .start_forever()
        .await
        .expect("http server error");
}
