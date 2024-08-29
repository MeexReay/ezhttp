use ezhttp::{Headers, HttpRequest, HttpResponse, HttpServer, HttpServerStarter};
use std::{
    io::{stdin, stdout, Error, Write},
    time::Duration,
};

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
    async fn on_request(&self, req: &HttpRequest) -> Option<HttpResponse> {
        // println!("{} > {} {}", req.addr, req.method, req.page);

        if req.page == "/" {
            Some(HttpResponse::from_string(
                Headers::from(vec![("Content-Type", "text/html")]), // response headers
                "200 OK",                                           // response status code
                &self.index_page,                                   // response body
            ))
        } else {
            None // close connection
        }
    }

    async fn on_start(&self, _: &str) {
        // println!("Http server started on {}", host);
    }

    async fn on_close(&self) {
        // println!("Http server closed");
    }
}

fn input(prompt: &str) -> Result<String, Error> {
    stdout().write_all(prompt.as_bytes())?;
    stdout().flush()?;
    let mut buf = String::new();
    stdin().read_line(&mut buf)?;
    Ok(buf)
}

fn main() {
    let site_1 = HttpServerStarter::new(EzSite::new("Hello World! site_1"), "localhost:8080")
        .timeout(Some(Duration::from_secs(5))) // read & write timeout
        .threads(5) // threadpool size
        .start();

    let site_2 = HttpServerStarter::new(EzSite::new("Hello World! site_2"), "localhost:8081")
        .timeout(Some(Duration::from_secs(5))) // read & write timeout
        .threads(5) // threadpool size
        .start();

    input("enter to close site_1").unwrap();

    site_1.close();

    input("enter to close site_2").unwrap();

    site_2.close();
}
