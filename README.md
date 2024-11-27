# EzHttp
Easy http server for small sites

This library is under developement, so if you found any bugs, please write them to [Issues](https://github.com/MeexReay/ezhttp/issues)

## Setup

```toml
ezhttp = "0.2.0" # stable
ezhttp = { git = "https://github.com/MeexReay/ezhttp" } # unstable
```

## Examples

Hello world example:
```rust
use ezhttp::prelude::*;

struct EzSite(String);

impl HttpServer for EzSite {
    async fn on_request(&self, req: &HttpRequest) -> Option<HttpResponse> {
        println!("{} > {} {}", req.addr, req.method, req.url.to_path_string());

        if req.url.path == "/" {
            Some(HttpResponse::new(
                OK,                                                                // response status code
                Headers::from(vec![                                                // response headers
                    ("Content-Type", "text/html"),                                 // - content type
                    ("Content-Length", self.0.len().to_string().as_str())          // - content length
                ]), Body::from_text(&self.0.clone()),                              // response body
            ))
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
```

[More examples](https://github.com/MeexReay/ezhttp/blob/main/examples)

### Contributing

If you would like to contribute to the project, feel free to fork the repository and submit a pull request.

### License
This project is licensed under the WTFPL License
