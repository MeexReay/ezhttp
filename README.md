# EzHttp
Simple async http library with client and server

This library is under developement, so if you found any bugs, please write them to [Issues](https://git.meex.lol/MeexReay/ezhttp/issues)

## Setup

```toml
ezhttp = "0.2.1" # stable
ezhttp = { git = "https://git.meex.lol/MeexReay/ezhttp" } # unstable
```

## Examples

Client example:
```rust
use ezhttp::prelude::*;

#[tokio::main]
async fn main() -> Result<(), HttpError> {
    let client = HttpClient::builder().build(); // or HttpClient::default() 

    let url = URL::from_str("https://google.com")?;
    let request: HttpRequest = RequestBuilder::get(url).build();

    let response: HttpResponse = client.send(request).await?;

    println!("response status: {}", response.status_code);
    println!("response body: {} bytes", response.body.as_text().unwrap().len());
    
    Ok(())
}
```

Site example:
```rust
use ezhttp::prelude::*;

struct EzSite(String);

impl HttpServer for EzSite {
    async fn on_request(&self, req: &HttpRequest) -> Option<HttpResponse> {
        println!("{} > {} {}", req.addr, req.method, req.url.to_path_string());

        if req.url.path == "/" {
            Some(HttpResponse::new(
                OK,                                                       // response status code
                Headers::from(vec![                                       // response headers
                    ("Content-Type", "text/html"),                        // - content type
                    ("Content-Length", self.0.len().to_string().as_str()) // - content length
                ]), Body::from_text(&self.0.clone()),                     // response body
            ))
        } else {
            None // close connection
        }
    }

    async fn on_start(&self, host: &str) {
        println!("Http server started on {host}");
    }

    async fn on_close(&self) {
        println!("Http server closed");
    }
}

#[tokio::main]
async fn main() {
    HttpServerStarter::new(
            EzSite("Hello World!".to_string()), 
            "localhost:8080"
        ).timeout(Some(Duration::from_secs(5)))
        .threads(5)
        .start_forever()
        .await
        .expect("http server error");
}
```

[More examples](https://git.meex.lol/MeexReay/ezhttp/src/branch/main/examples)

### Contributing

If you would like to contribute to the project, feel free to fork the repository and submit a pull request.

### License
This project is licensed under the WTFPL License
