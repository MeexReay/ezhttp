use std::{error::Error, str::FromStr, time::Duration};

use ezhttp::{client::{ClientBuilder, RequestBuilder}, request::URL};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let client = ClientBuilder::new()
        .ssl_verify(false)
        .connect_timeout(Duration::from_secs(5))
        .write_timeout(Duration::from_secs(5))
        .read_timeout(Duration::from_secs(5))
        .header("User-Agent", "EzHttp/0.1.0")
        .build();

    let request = RequestBuilder::get(URL::from_str("https://meex.lol/dku?key=value#hex_id")?).build();

    println!("request: {:?}", &request);

    let response = client.send(request).await?;

    println!("response: {:?}", &response);

    Ok(())
}
