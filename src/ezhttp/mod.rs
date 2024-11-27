pub mod error;
pub mod headers;
pub mod request;
pub mod response;
pub mod body;
pub mod server;
pub mod client;

pub mod prelude {
    pub use super::error::*;
    pub use super::headers::*;
    pub use super::request::*;
    pub use super::response::*;
    pub use super::response::status_code::*;
    pub use super::body::*;
    pub use super::server::*;
    pub use super::server::handler::*;
    pub use super::server::starter::*;
    pub use super::client::*;
}

use error::HttpError;
use rand::Rng;
use tokio::{io::AsyncReadExt, net::TcpStream};
use tokio_io_timeout::TimeoutStream;

const CHARS: &str = "qwertyuiopasdfghjklzxcvbnm0123456789QWERTYUIOPASDFGHJKLZXCVBNM'()+_,-./:=?";

pub fn gen_multipart_boundary() -> String {
    let range = 20..40;
    let length: usize = rand::thread_rng().gen_range(range);
    [0..length].iter().map(|_|
        String::from(CHARS.chars()
            .collect::<Vec<char>>()
            .get(rand::thread_rng()
                .gen_range(0..CHARS.len())
            ).unwrap().clone()
        )
    ).collect::<Vec<String>>().join("")
}

fn split_bytes_once(bytes: &[u8], sep: &[u8]) -> (Vec<u8>, Vec<u8>) {
    if let Some(index) = bytes.windows(sep.len())
            .enumerate()
            .filter(|o| o.1 == sep)
            .map(|o| o.0)
            .next() {
        let t = bytes.split_at(index);
        (t.0.to_vec(), t.1.split_at(sep.len()).1.to_vec())
    } else {
        (Vec::from(bytes), Vec::new())
    }
}

fn split_bytes(bytes: &[u8], sep: &[u8]) -> Vec<Vec<u8>> {
    if bytes.len() >= sep.len() {
        let indexes: Vec<usize> = bytes.windows(sep.len())
            .enumerate()
            .filter(|o| o.1 == sep)
            .map(|o| o.0)
            .collect();
        let mut parts: Vec<Vec<u8>> = Vec::new();
        let mut now_part: Vec<u8> = Vec::new();
        let mut i = 0usize;
        loop {
            if i >= bytes.len() {
                break;
            }

            if indexes.contains(&i) {
                parts.push(now_part.clone());
                now_part.clear();
                i += sep.len();
                continue;
            }
            
            now_part.push(bytes[i]);

            i += 1;
        }
        parts.push(now_part);
        parts
    } else {
        vec![Vec::from(bytes)]
    }
}

async fn read_line(data: &mut (impl AsyncReadExt + Unpin)) -> Result<String, HttpError> {
    let mut line = Vec::new();
    loop {
        let mut buffer = vec![0;1];
        data.read_exact(&mut buffer).await.or(Err(HttpError::ReadLineEof))?;
        let char = buffer[0];
        line.push(char);
        if char == 0x0a {
            break;
        }
    }
    String::from_utf8(line).or(Err(HttpError::ReadLineUnknown))
}

async fn read_line_crlf(data: &mut (impl AsyncReadExt + Unpin)) -> Result<String, HttpError> {
    match read_line(data).await {
        Ok(i) => Ok(i[..i.len() - 2].to_string()),
        Err(e) => Err(e),
    }
}

pub type Stream = TimeoutStream<TcpStream>;