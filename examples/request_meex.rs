use std::{error::Error, time::Duration};

use ezhttp::{client::{ClientBuilder, RequestBuilder}, request::IntoURL};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dbg!("https://meex.lol/dku?key=value#hex_id".to_url().unwrap().to_string());

    let client = ClientBuilder::new()
        .ssl_verify(false)
        .connect_timeout(Duration::from_secs(5))
        .write_timeout(Duration::from_secs(5))
        .read_timeout(Duration::from_secs(5))
        .header("User-Agent", "EzHttp/0.1.0")
        .build();

    let request = RequestBuilder::get("https://meex.lol/dku?key=value#hex_id");

    println!("request: {:?}", &request);

    let response = client.send(request).await?;

    println!("response: {:?}", &response);

    Ok(())
}
