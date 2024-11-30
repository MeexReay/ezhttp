use std::{collections::HashMap, path::PathBuf};

use async_trait::async_trait;
use serde_json::Value;
use tokio::{fs, io::{AsyncReadExt, AsyncWriteExt}};

use crate::ezhttp::{split_bytes, split_bytes_once};

use super::{error::HttpError, headers::Headers, read_line_crlf, Sendable};

#[derive(Debug, Clone)]
pub struct Body {
    pub data: Vec<u8>
}

impl Body {
    pub fn new(data: Vec<u8>) -> Body {
        Body {
            data
        }
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        self.data.clone()
    }

    pub fn as_text(&self) -> Option<String> {
        String::from_utf8(self.data.clone()).ok()
    }

    pub fn as_query(&self) -> Option<HashMap<String, String>> {
        let mut text = self.as_text()?;
        if text.starts_with("?") {
            text = text[1..].to_string();
        }
        Some(HashMap::from_iter(text.split("&").filter_map(|entry| {
            let (key, value) = entry.split_once("=").unwrap_or((entry, ""));
            Some((urlencoding::decode(key).ok()?.to_string(), urlencoding::decode(value).ok()?.to_string()))
        })))
    }

    pub fn as_json(&self) -> Option<Value> {
        serde_json::from_str(&self.as_text()?).ok()
    }

    pub fn from_bytes(bytes: &[u8]) -> Body {
        Self::new(bytes.to_vec())
    }

    pub fn from_text(text: &str) -> Body {
        Self::from_bytes(text.as_bytes())
    }

    pub fn from_query(params: HashMap<String, String>) -> Body {
        Self::from_text(&params.iter()
            .map(|o| 
                format!("{}={}", 
                    urlencoding::encode(o.0), 
                    urlencoding::encode(o.1))
            )
            .collect::<Vec<String>>()
            .join("&")
        )
    }

    pub fn from_json(value: Value) -> Body {
        Self::from_text(&value.to_string())
    }

    pub fn from_multipart(parts: Vec<Part>, boundary: String) -> Body {
        let mut data: Vec<u8> = Vec::new();

        for part in parts {
            data.append(&mut b"--".to_vec());
            data.append(&mut boundary.as_bytes().to_vec());
            data.append(&mut b"\r\nContent-Disposition: form-data; name=\"".to_vec());
            data.append(&mut part.name.as_bytes().to_vec());
            data.append(&mut b"\"".to_vec());
            if let Some(filename) = &part.filename {
                data.append(&mut b"; filename=\"".to_vec());
                data.append(&mut filename.as_bytes().to_vec());
                data.append(&mut b"\"".to_vec());
            }
            data.append(&mut b"\r\n".to_vec());
            if let Some(content_type) = &part.content_type {
                data.append(&mut b"Content-Type: ".to_vec());
                data.append(&mut content_type.as_bytes().to_vec());
                data.append(&mut b"\r\n".to_vec());
            }
            data.append(&mut b"\r\n".to_vec());
            data.append(&mut part.body.as_bytes());
        }

        data.append(&mut b"--".to_vec());
        data.append(&mut boundary.as_bytes().to_vec());
        data.append(&mut b"--\r\n".to_vec());

        Self::from_bytes(&data)
    }

    pub fn as_multipart(&self, boundary: String) -> Vec<Part> {
        let data = self.as_bytes();
        split_bytes(&data, format!("--{boundary}").as_bytes()).iter()
            .filter(|o| o != &b"--\r\n\r\n")
            .filter_map(|o| {
                let (head,body) = split_bytes_once(o, b"\r\n\r\n");
                let head = String::from_utf8(head).ok()?;
                let head = head.split("\r\n").filter_map(|h| {
                    let (name, value) = h.split_once(": ")?;
                    Some((name.to_lowercase(), value.to_string()))
                }).collect::<Vec<(String, String)>>();
                let content_type = head.iter()
                    .find(|o| o.0 == "content-type")
                    .map(|o| o.1.clone());
                let (name, filename) = head.iter()
                    .find(|o| o.0 == "content-disposition")
                    .map(|o| o.1.split(";")
                        .filter(|o| o != &"form-data")
                        .map(|s| s.trim().to_string())
                        .collect::<Vec<String>>())
                    .map(|o| (
                        o.iter().find(|k| k.starts_with("name=\"")).map(|k| k[6..k.len()-1].to_string()), 
                        o.iter().find(|k| k.starts_with("filename=\"")).map(|k| k[10..k.len()-1].to_string())
                    ))?;
                let name = name?;

                Some(Part::new(name, Body::from_bytes(&body), filename, content_type))
            }).collect::<Vec<Part>>()
    }

    pub async fn recv(stream: &mut (impl AsyncReadExt + Unpin), headers: &Headers) -> Result<Body, HttpError> {
        let mut reqdata: Vec<u8> = Vec::new();

        if let Some(content_size) = headers.clone().get("content-length".to_string()) {
            let content_size: usize = content_size.parse().map_err(|_| HttpError::InvalidContentSize)?;
            reqdata.resize(content_size, 0);
            stream.read_exact(&mut reqdata).await.map_err(|_| HttpError::InvalidContent)?;
        } else if let Some(transfer_encoding) = headers.clone().get("transfer_encoding".to_string()) {
            if transfer_encoding.split(",").map(|o| o.trim()).find(|o| o == &"chunked").is_some() {
                loop {
                    let length = usize::from_str_radix(&read_line_crlf(stream).await?, 16).map_err(|_| HttpError::InvalidContent)?;
                    if length == 0 { break }
                    let mut data = vec![0u8; length+2];
                    stream.read_exact(&mut data).await.map_err(|_| HttpError::InvalidContent)?;
                    data.truncate(length);
                    reqdata.append(&mut data);
                }
            }
        }

        Ok(Body::from_bytes(&reqdata))
    }
}

#[async_trait]
impl Sendable for Body {
    async fn send(&self, stream: &mut (impl AsyncWriteExt + Unpin + Send)) -> Result<(), HttpError> {
        stream.write_all(&self.as_bytes()).await.map_err(|_| HttpError::WriteHeadError)
    }
}

impl Default for Body {
    fn default() -> Self {
        Body {
            data: Vec::new()
        }
    }
}

#[derive(Clone,Debug)]
pub struct Part {
    pub name: String,
    pub body: Body,
    pub filename: Option<String>,
    pub content_type: Option<String>
}

impl Part {
    pub fn new(
        name: String,
        body: Body,
        filename: Option<String>,
        content_type: Option<String>
    ) -> Part {
        Part {
            name,
            body,
            filename,
            content_type
        }
    }

    pub fn body(name: String, body: Body) -> Part {
        Part {
            name,
            body,
            filename: None,
            content_type: None
        }
    }

    pub async fn file(name: String, file: PathBuf) -> Option<Part> {
        Some(Part {
            name,
            body: Body::from_text(&fs::read_to_string(&file).await.ok()?),
            filename: Some(file.file_name()?.to_str()?.to_string()),
            content_type: mime_guess::from_path(file).first().map(|o| o.to_string())
        })
    }
}

