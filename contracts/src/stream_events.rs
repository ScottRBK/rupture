use serde::{Deserialize, Serialize};

#[derive(Serialize,Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TextStreamEvent {
    Chunk { text: String, token_count: i32, is_reasoning: bool },
    Error { message: String }
}
