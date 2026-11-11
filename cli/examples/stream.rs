use anyhow::Result;
use async_stream::try_stream;
use futures::stream::StreamExt;
use futures_core::Stream;
use reqwest::Client;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<()> {

    let prompt = "hey there, tell me a story".to_string();

    let stream = text_prompt(prompt).await;

    futures::pin_mut!(stream);
    let mut line_buffer = String::new();
    while let Some(chunk) = stream.next().await {
        line_buffer.push_str(&chunk?);

        while let Some(newline_index) = line_buffer.find("\n") {
            let line: String = line_buffer.drain(..=newline_index).collect();
            print!("{line}");
        }

    }

    if !line_buffer.is_empty() {
        println!("{line_buffer}");
    }

    Ok(())

}

pub async fn text_prompt(prompt: String) -> impl Stream<Item = Result<String>> + Send + 'static {
    try_stream! {
        let client = Client::new();

        let mut map = HashMap::new();
        map.insert("prompt", prompt);

        let res = client.post("http://localhost:3000/prompt")
            .json(&map)
            .send()
            .await?;

        let mut stream = res.bytes_stream();

        let mut buffer: Vec<u8> = Vec::new();
        let mut event_data = String::new();

        while let Some(chunk) = stream.next().await {
            buffer.extend_from_slice(&chunk?);
            while let Some(n1) = buffer.iter().position(|&b| b == b'\n') {
                let line_bytes: Vec<u8> = buffer.drain(..=n1).collect();
                let line = String::from_utf8_lossy(&line_bytes);
                let line = line.trim_end_matches(&['\r', '\n'][..]);  

                if line.is_empty() {
                    if !event_data.is_empty() {
                        event_data.pop();
                        yield std::mem::take(&mut event_data);
                    }

                    continue;
                }

                if let Some(data) = line.strip_prefix("data:") {
                    let data = data.strip_prefix(' ').unwrap_or(data);

                    event_data.push_str(data);
                    event_data.push('\n');
                }
            }
        }
    }
}

