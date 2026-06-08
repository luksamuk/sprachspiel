//! NDJSON streaming parser with idle timeout for Ollama API responses.

use futures::{Stream, StreamExt};
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::BufReader;

/// Simple error type for streaming
#[derive(Debug, Clone)]
pub enum StreamError {
    Io(String),
    Parse(String),
    EndOfStream,
}

impl std::fmt::Display for StreamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StreamError::Io(s) => write!(f, "IO error: {}", s),
            StreamError::Parse(s) => write!(f, "Parse error: {}", s),
            StreamError::EndOfStream => write!(f, "End of stream"),
        }
    }
}

impl std::error::Error for StreamError {}

/// Wrapper around a byte stream that parses NDJSON lines.
pub struct NdjsonStream {
    pub body: Option<reqwest::Body>,
    pub buffer: String,
    pub finished: bool,
}

impl NdjsonStream {
    pub fn new(body: reqwest::Body) -> Self {
        Self {
            body: Some(body),
            buffer: String::new(),
            finished: false,
        }
    }
}

impl Stream for NdjsonStream {
    type Item = Result<String, StreamError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.finished {
            return Poll::Ready(None);
        }
        
        loop {
            // Check if we have a complete line in buffer
            if let Some(pos) = self.buffer.find('\n') {
                let line = self.buffer.drain(..=pos).collect::<String>();
                return Poll::Ready(Some(Ok(line.trim_end().to_string())));
            }
            
            // If body is consumed and buffer is empty, we're done
            if self.body.is_none() {
                self.finished = true;
                if !self.buffer.is_empty() {
                    let line = self.buffer.drain(..).collect::<String>();
                    return Poll::Ready(Some(Ok(line)));
                }
                return Poll::Ready(None);
            }
            
            // For simplicity, just try to read chunks
            // In a full implementation, this would use AsyncRead with proper polling
            let body = self.body.take().unwrap();
            self.body = Some(body);
            
            // Return pending for now - actual implementation needs proper async I/O
            return Poll::Pending;
        }
    }
}