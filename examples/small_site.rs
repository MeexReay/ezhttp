use async_trait::async_trait;
use ezhttp::{prelude::*, Sendable};

struct EzSite(String);

#[async_trait]
impl HttpServer for EzSite {
    async fn on_request(&self, req: &HttpRequest) -> Option<Box<dyn Sendable>> {
        println!("{} > {} {}", req.addr?, req.method, req.url.to_string());

        if req.url.path == "/" {
            Some(HttpResponse::new(
                OK,                                                    // response status code
                Headers::from(vec![                                                // response headers
                    ("Content-Type", "text/html"),                                 // - content type
                    ("Content-Length", self.0.len().to_string().as_str())          // - content length
                ]), Body::from_text(&self.0.clone()),                              // response body
            ).as_box())
        } else {
            None // close connection
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
    start_server(EzSite("Hello World!".to_string()), "localhost:8080").await.expect("http server error");
}
