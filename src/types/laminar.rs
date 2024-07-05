use async_openai::types::ChatCompletionRequestMessage;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize, Debug)]
pub struct LaminarRequestArgs {
    pub endpoint: String,
    pub inputs: Value,
    pub env: Value,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LaminarValue {
    pub value: String
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SimpleLLMQueryInputs {
    pub prompt: String
}

impl Into<Value> for SimpleLLMQueryInputs {
    fn into(self) -> Value {
        serde_json::json!({
            "prompt": self.prompt
        })
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SimpleLLMQueryOutputs {
    pub output: LaminarValue
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChatAgentInputs {
    pub messages: Vec<ChatCompletionRequestMessage>,
    pub user_memories: String,
    pub date: String
}

impl Into<Value> for ChatAgentInputs {
    fn into(self) -> Value {
        serde_json::json!({
            "chat_messages": self.messages,
            "memories": self.user_memories,
            "date": self.date
        })
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ChatAgentOutputs {
    pub response: LaminarValue
}

#[derive(Serialize, Deserialize, Debug)]
pub enum LaminarEndpoints {
    SimpleLLMQuery(SimpleLLMQueryInputs),
    ChatAgentQuery(ChatAgentInputs)
}

#[derive(Serialize, Deserialize, Debug)]
pub enum LaminarOutputs {
    SimpleLLMQuery(SimpleLLMQueryOutputs),
    ChatAgentQuery(ChatAgentOutputs)
}