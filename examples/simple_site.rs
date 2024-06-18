use ezhttp::{Headers, HttpRequest, HttpResponse, HttpServer};

struct EzSite {
    main_page: String,
}

impl EzSite {
    fn new(index_page: &str) -> Self {
        EzSite {
            main_page: index_page.to_string(),
        }
    }

    fn ok_response(&mut self, content: String) -> HttpResponse {
        HttpResponse::from_string(
            Headers::from(vec![("Content-Type", "text/html")]),
            "200 OK".to_string(),
            content,
        )
    }

    fn not_found_response(&mut self, content: String) -> HttpResponse {
        HttpResponse::from_string(
            Headers::from(vec![("Content-Type", "text/html")]),
            "404 Not Found".to_string(),
            content,
        )
    }

    async fn get_main_page(&mut self, req: &HttpRequest) -> Option<HttpResponse> {
        if req.page == "/" {
            Some(self.ok_response(self.main_page.clone()))
        } else {
            None
        }
    }

    async fn get_unknown_page(&mut self, req: &HttpRequest) -> Option<HttpResponse> {
        Some(self.not_found_response(format!("<h1>404 Error</h1>Not Found {}", &req.page)))
    }
}

impl HttpServer for EzSite {
    async fn on_request(&mut self, req: &HttpRequest) -> Option<HttpResponse> {
        println!("{} > {} {}", req.addr, req.method, req.page);

        if let Some(resp) = self.get_main_page(req).await {
            Some(resp)
        } else if let Some(resp) = self.get_unknown_page(req).await {
            Some(resp)
        } else {
            None // shutdown socket
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
    let site = EzSite::new("<h1>Hello World!</h1>");
    let host = "localhost:8080";

    ezhttp::start_server(site, host).unwrap();
}
