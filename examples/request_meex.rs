use std::str::FromStr;

use ezhttp::{client::{HttpClient, RequestBuilder}, request::URL};

#[tokio::main]
async fn main() {
    let response = HttpClient::default().send(
        RequestBuilder::get(
            URL::from_str("https://meex.lol/dku?key=value#hex_id")
                .expect("url error")
            ).build()
        ).await.expect("request error");
    println!("status code: {}", response.status_code);
    println!("headers: {}", response.headers.entries().iter().map(|o| format!("{}: {}", o.0, o.1)).collect::<Vec<String>>().join("; "));
    println!("body: {} bytes", response.body.as_text().unwrap().len());
}
