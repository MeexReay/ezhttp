use std::time::Duration;

use ezhttp::{
    body::Body, 
    headers::Headers, 
    request::HttpRequest, 
    response::{
        status_code::{NOT_FOUND, OK}, 
        HttpResponse
    }, 
    server::{
        starter::HttpServerStarter, 
        HttpServer
    }, Sendable
};

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
        HttpResponse::new(
            OK,
            Headers::from(vec![
                ("Content-Length", content.len().to_string().as_str()),
                ("Content-Type", "text/html"),
                ("Connection", "keep-alive"),
            ]),
            Body::from_text(&content),
        )
    }

    fn not_found_response(&self, content: String) -> HttpResponse {
        HttpResponse::new(
            NOT_FOUND,
            Headers::from(vec![
                ("Content-Length", content.len().to_string().as_str()),
                ("Content-Type", "text/html"),
                ("Connection", "keep-alive"),
            ]),
            Body::from_text(&content),
        )
    }

    async fn get_main_page(&self, req: &HttpRequest) -> Option<HttpResponse> {
        if req.url.path == "/" {
            Some(self.ok_response(self.main_page.clone()))
        } else {
            None
        }
    }

    async fn get_unknown_page(&self, req: &HttpRequest) -> Option<HttpResponse> {
        Some(self.not_found_response(format!("<h1>404 Error</h1>Not Found {}", &req.url.path)))
    }
}

impl HttpServer for EzSite {
    async fn on_request(&self, req: &HttpRequest) -> Option<impl Sendable> {
        println!("{} > {} {}", req.addr, req.method, req.url.to_path_string());

        if let Some(resp) = self.get_main_page(req).await {
            Some(resp)
        } else if let Some(resp) = self.get_unknown_page(req).await {
            Some(resp)
        } else {
            None // shutdown connection
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
    let host = "localhost:8000";

    HttpServerStarter::new(site, host)
        .timeout(Some(Duration::from_secs(5)))
        .threads(5)
        .start_forever()
        .await
        .expect("http server error");
}
