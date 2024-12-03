use std::{
    collections::HashMap,
    fmt::{Debug, Display},
};

use async_trait::async_trait;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::{error::HttpError, read_line_crlf, Sendable};

/// Http headers
#[derive(Clone, Debug)]
pub struct Headers {
    entries: Vec<(String, String)>,
}

impl Into<HashMap<String, String>> for Headers {
    fn into(self) -> HashMap<String, String> {
        HashMap::from_iter(self.entries().into_iter())
    }
}

impl<T, U> From<Vec<(T, U)>> for Headers
where
    T: ToString,
    U: ToString,
{
    fn from(value: Vec<(T, U)>) -> Self {
        Headers {
            entries: value
                .into_iter()
                .map(|v| (v.0.to_string(), v.1.to_string()))
                .collect(),
        }
    }
}

impl Headers {
    pub fn new() -> Self {
        Headers {
            entries: Vec::new(),
        }
    }

    pub fn contains(&self, header: impl ToString) -> bool {
        for (k, _) in &self.entries {
            if k.to_lowercase() == header.to_string().to_lowercase() {
                return true;
            }
        }
        return false;
    }

    pub fn get(&self, key: impl ToString) -> Option<String> {
        for (k, v) in &self.entries {
            if k.to_lowercase() == key.to_string().to_lowercase() {
                return Some(v.clone());
            }
        }
        return None;
    }

    pub fn put(&mut self, key: impl ToString, value: String) {
        for t in self.entries.iter_mut() {
            if t.0.to_lowercase() == key.to_string().to_lowercase() {
                t.1 = value;
                return;
            }
        }
        self.entries.push((key.to_string(), value));
    }

    pub fn put_default(&mut self, key: impl ToString, value: String) {
        for t in self.entries.iter_mut() {
            if t.0.to_lowercase() == key.to_string().to_lowercase() {
                return;
            }
        }
        self.entries.push((key.to_string(), value));
    }

    pub fn remove(&mut self, key: impl ToString) {
        for (i, t) in self.entries.iter_mut().enumerate() {
            if t.0.to_lowercase() == key.to_string().to_lowercase() {
                self.entries.remove(i);
                return;
            }
        }
    }

    pub fn keys(&self) -> Vec<String> {
        self.entries.iter().map(|e| e.0.clone()).collect()
    }

    pub fn values(&self) -> Vec<String> {
        self.entries.iter().map(|e| e.1.clone()).collect()
    }

    pub fn entries(&self) -> Vec<(String, String)> {
        return self.entries.clone();
    }

    pub fn len(&self) -> usize {
        return self.entries.len();
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub async fn recv(stream: &mut (impl AsyncReadExt + Unpin)) -> Result<Headers, HttpError> {
        let mut headers = Headers::new();

        loop {
            let text = read_line_crlf(stream).await.map_err(|_| HttpError::InvalidHeaders)?;
            if text.len() == 0 { break }

            let (key, value) = text.split_once(": ").ok_or(HttpError::InvalidHeaders)?;
            headers.put(key.to_lowercase(), value.to_string());
        }

        Ok(headers)
    }
}

impl Display for Headers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self, f)
    }
}

#[async_trait]
impl Sendable for Headers {
    async fn send(&self, stream: &mut (impl AsyncWriteExt + Unpin + Send)) -> Result<(), HttpError> {
        let mut head = String::new();
        for (k, v) in self.entries() {
            head.push_str(&k);
            head.push_str(": ");
            head.push_str(&v);
            head.push_str("\r\n");
        }
        stream.write_all(head.as_bytes()).await.map_err(|_| HttpError::WriteHeadError)
    }
}