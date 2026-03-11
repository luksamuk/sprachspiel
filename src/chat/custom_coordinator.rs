//! Custom Coordinator with callbacks for intermediate content
//!
//! This is our own implementation that provides:
//! - Pre-tool content callbacks (content before tool calls)
//! - Thinking content callbacks
//! - Full control over tool execution flow

use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
};

use ollama_rs::{
    generation::{
        chat::{request::ChatMessageRequest, ChatMessage, ChatMessageResponse, MessageRole},
        parameters::{FormatType, KeepAlive, ThinkType},
        tools::Tool,
    },
    history::ChatHistory,
    models::ModelOptions,
    re_exports::schemars::{generate::SchemaSettings, JsonSchema, Schema},
    Ollama,
};
use ollama_rs::re_exports::serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;

/// Result type for tool execution
pub type ToolResult = std::result::Result<String, Box<dyn std::error::Error + Send + Sync>>;

/// Trait to hold and call tools - our own implementation since ollama-rs's ToolHolder is private
pub trait ToolHolder: Send + Sync {
    fn call(&mut self, parameters: Value) -> Pin<Box<dyn Future<Output = ToolResult> + '_ + Send + Sync>>;
}

impl<T: Tool> ToolHolder for T {
    fn call(&mut self, parameters: Value) -> Pin<Box<dyn Future<Output = ToolResult> + '_ + Send + Sync>> {
        Box::pin(async move {
            // Handle different JSON formats that models might return
            let param_value = match serde_json::from_value::<ToolCallFunctionParser>(parameters.clone()) {
                Ok(func) => func.arguments,
                Err(_) => parameters,
            };

            let params: T::Params = serde_json::from_value(param_value)?;
            T::call(self, params).await
        })
    }
}

/// Helper struct for parsing tool call function JSON
#[derive(Deserialize)]
struct ToolCallFunctionParser {
    #[allow(dead_code)]
    name: String,
    arguments: Value,
}

/// Tool type enum
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CustomToolType {
    #[serde(rename_all(deserialize = "PascalCase"))]
    Function,
}

/// Tool function info
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CustomToolFunctionInfo {
    pub name: String,
    pub description: String,
    pub parameters: Schema,
}

/// Our own ToolInfo since ollama-rs's ToolInfo::new is private
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CustomToolInfo {
    #[serde(rename = "type")]
    pub tool_type: CustomToolType,
    pub function: CustomToolFunctionInfo,
}

impl CustomToolInfo {
    /// Create a new ToolInfo for a given Tool type
    pub fn new<P: DeserializeOwned + JsonSchema, T: Tool<Params = P>>() -> Self {
        let mut settings = SchemaSettings::draft07();
        settings.inline_subschemas = true;
        let generator = settings.into_generator();

        let parameters = generator.into_root_schema_for::<P>();

        Self {
            tool_type: CustomToolType::Function,
            function: CustomToolFunctionInfo {
                name: T::name().to_string(),
                description: T::description().to_string(),
                parameters,
            },
        }
    }
}

/// Event emitted during chat processing
#[derive(Debug, Clone)]
pub enum ChatEvent {
    /// Content generated before tool calls (often thinking/intro text)
    PreToolContent {
        content: String,
        thinking: Option<String>,
    },
    /// Tool call is about to be executed (fields kept for future use)
    #[allow(dead_code)]
    ToolCall {
        name: String,
        arguments: serde_json::Value,
    },
    /// Tool execution result (name kept for future debugging use)
    #[allow(dead_code)]
    ToolResult { name: String, result: String },
    /// Final response (no more tool calls) - kept for future use
    #[allow(dead_code)]
    FinalResponse(ChatMessageResponse),
}

/// A coordinator for managing chat interactions with event callbacks.
pub struct CustomCoordinator<C: ChatHistory> {
    model: String,
    ollama: Ollama,
    options: ModelOptions,
    history: C,
    tool_infos: Vec<CustomToolInfo>,
    tools: HashMap<String, Box<dyn ToolHolder>>,
    debug: bool,
    format: Option<FormatType>,
    keep_alive: Option<KeepAlive>,
    think: Option<ThinkType>,
    /// Callback for events
    event_callback: Option<Box<dyn Fn(ChatEvent) + Send + Sync>>,
    /// Context window size for overflow detection during tool execution
    context_window: Option<usize>,
    /// System prompt for token estimation
    system_prompt: Option<String>,
}

impl<C: ChatHistory> CustomCoordinator<C> {
    /// Creates a new `CustomCoordinator` instance.
    pub fn new(ollama: Ollama, model: String, history: C) -> Self {
        Self {
            model,
            ollama,
            options: ModelOptions::default(),
            history,
            tool_infos: Vec::default(),
            tools: HashMap::default(),
            debug: false,
            format: None,
            keep_alive: None,
            think: None,
            event_callback: None,
            context_window: None,
            system_prompt: None,
        }
    }

    /// Set the context window size for overflow detection
    pub fn context_window(mut self, context_window: usize) -> Self {
        self.context_window = Some(context_window);
        self
    }

    /// Set the system prompt for token estimation
    pub fn system_prompt(mut self, system_prompt: String) -> Self {
        self.system_prompt = Some(system_prompt);
        self
    }

    /// Add a tool to the coordinator
    pub fn add_tool<T: Tool + 'static>(mut self, tool: T) -> Self {
        self.tool_infos.push(CustomToolInfo::new::<T::Params, T>());
        self.tools.insert(T::name().to_string(), Box::new(tool));
        self
    }

    /// Set the format (for future use)
    #[allow(dead_code)]
    pub fn format(mut self, format: FormatType) -> Self {
        self.format = Some(format);
        self
    }

    /// Set model options
    pub fn options(mut self, options: ModelOptions) -> Self {
        self.options = options;
        self
    }

    /// Enable debug mode (for future use)
    #[allow(dead_code)]
    pub fn debug(mut self, debug: bool) -> Self {
        self.debug = debug;
        self
    }

    /// Set keep alive (for future use)
    #[allow(dead_code)]
    pub fn keep_alive(mut self, keep_alive: KeepAlive) -> Self {
        self.keep_alive = Some(keep_alive);
        self
    }

    /// Set think mode
    pub fn think(mut self, think: impl Into<ThinkType>) -> Self {
        self.think = Some(think.into());
        self
    }

    /// Set event callback for receiving intermediate content and events
    pub fn on_event(mut self, callback: impl Fn(ChatEvent) + Send + Sync + 'static) -> Self {
        self.event_callback = Some(Box::new(callback));
        self
    }

    /// Emit an event if callback is set
    fn emit_event(&self, event: ChatEvent) {
        if let Some(ref callback) = self.event_callback {
            callback(event);
        }
    }

    /// Send a chat message and process tool calls with event emission
    pub async fn chat(
        &mut self,
        messages: Vec<ChatMessage>,
    ) -> ollama_rs::error::Result<ChatMessageResponse> {
        if self.debug {
            for m in &messages {
                eprintln!("Hit {} with:", self.model);
                eprintln!("\t{:?}: '{}'", m.role, m.content);
            }
        }

        let request = ChatMessageRequest::new(self.model.clone(), messages)
            .options(self.options.clone());

        // Apply optional settings
        let request = self.apply_optional_settings(request);

        // Push initial messages to history
        for m in request.messages.clone() {
            self.history.push(m);
        }

        // Make the request
        let resp = self
            .ollama
            .send_chat_messages(self.build_request())
            .await?;

        // Process the response
        self.process_response(resp).await
    }

    /// Apply optional settings to request
    fn apply_optional_settings(&self, mut request: ChatMessageRequest) -> ChatMessageRequest {
        if let Some(ref keep_alive) = self.keep_alive {
            request = request.keep_alive(keep_alive.clone());
        }

        if let Some(ref think) = self.think {
            request = request.think(think.clone());
        }

        if let Some(ref format) = self.format {
            if self.tool_infos.is_empty() {
                request = request.format(format.clone());
            } else if let Some(last_message) = self.history.messages().last()
                && last_message.role == MessageRole::Tool
            {
                request = request.format(format.clone());
            }
        }

        request
    }

    /// Build a request from current history
    fn build_request(&self) -> ChatMessageRequest {
        let mut request = ChatMessageRequest::new(self.model.clone(), self.history.messages().to_vec())
            .options(self.options.clone());

        // Add tools - need to convert our CustomToolInfo to ollama-rs's ToolInfo
        // We serialize ours and it's compatible
        let tools_json = serde_json::to_string(&self.tool_infos).unwrap_or_default();
        let tools: Vec<ollama_rs::generation::tools::ToolInfo> =
            serde_json::from_str(&tools_json).unwrap_or_default();
        request = request.tools(tools);

        if let Some(ref keep_alive) = self.keep_alive {
            request = request.keep_alive(keep_alive.clone());
        }

        if let Some(ref think) = self.think {
            request = request.think(think.clone());
        }

        if let Some(ref format) = self.format {
            if self.tool_infos.is_empty() {
                request = request.format(format.clone());
            } else if let Some(last_message) = self.history.messages().last()
                && last_message.role == MessageRole::Tool
            {
                request = request.format(format.clone());
            }
        }

        request
    }

    /// Process response, handling tool calls with event emission
    async fn process_response(
        &mut self,
        resp: ChatMessageResponse,
    ) -> ollama_rs::error::Result<ChatMessageResponse> {
        // Check if there are tool calls
        if !resp.message.tool_calls.is_empty() {
            // Emit pre-tool content if present
            let has_content = !resp.message.content.trim().is_empty();
            let has_thinking = resp.message.thinking.is_some();

            if has_content || has_thinking {
                self.emit_event(ChatEvent::PreToolContent {
                    content: resp.message.content.clone(),
                    thinking: resp.message.thinking.clone(),
                });
            }

            // Push assistant message to history (with tool calls)
            self.history.push(resp.message.clone());

            // Execute each tool call
            for call in resp.message.tool_calls {
                let tool_name = call.function.name.clone();
                let args = call.function.arguments.clone();

                // Emit tool call event
                self.emit_event(ChatEvent::ToolCall {
                    name: tool_name.clone(),
                    arguments: args.clone(),
                });

                if self.debug {
                    eprintln!("Tool call: {:?}", call.function);
                }

                let Some(tool) = self.tools.get_mut(&tool_name) else {
                    if self.debug {
                        eprintln!(
                            "\x1B[90m[DEBUG] Unknown tool '{}'. Available: {}\x1B[0m",
                            tool_name,
                            self.tools.keys().cloned().collect::<Vec<_>>().join(", ")
                        );
                    }
                    return Err(ollama_rs::error::OllamaError::ToolCallError(
                        ollama_rs::error::ToolCallError::UnknownToolName,
                    ));
                };

                let result = match tool.call(args.clone()).await {
                    Ok(result) => result,
                    Err(e) => {
                        if self.debug {
                            eprintln!(
                                "\x1B[90m[DEBUG] Tool '{}' call failed: {}\x1B[0m",
                                tool_name, e
                            );
                            eprintln!(
                                "\x1B[90m[DEBUG]   Arguments: {}\x1B[0m",
                                serde_json::to_string(&args).unwrap_or_else(|_| args.to_string())
                            );
                        }
                        return Err(ollama_rs::error::OllamaError::ToolCallError(
                            ollama_rs::error::ToolCallError::InternalToolError(e),
                        ));
                    }
                };

                if self.debug {
                    eprintln!("Tool response: {}", &result);
                }

                // Use full result (no truncation - LLM controls via head/tail)
                let result = result;

                // Emit tool result event
                self.emit_event(ChatEvent::ToolResult {
                    name: tool_name.clone(),
                    result: result.clone(),
                });

                // Push tool result to history
                self.history.push(ChatMessage::tool(result));
            }

            // Recurse to get next response
            Box::pin(self.process_next()).await
        } else {
            // No tool calls - this is the final response
            // Push to history
            self.history.push(resp.message.clone());

            // Emit final response event
            self.emit_event(ChatEvent::FinalResponse(resp.clone()));

            Ok(resp)
        }
    }

    /// Process next response after tool calls
    async fn process_next(&mut self) -> ollama_rs::error::Result<ChatMessageResponse> {
        // Check context overflow before sending to Ollama
        if let (Some(ctx_window), Some(prompt)) = (self.context_window, &self.system_prompt) {
            let history_tokens = crate::context_overflow::estimate_chat_messages_tokens(&self.history.messages());
            let system_tokens = crate::tokens::estimate_tokens(prompt) + crate::tokens::MESSAGE_OVERHEAD;
            let total_tokens = history_tokens + system_tokens;
            
            // Use 90% threshold to detect overflow early
            let threshold = (ctx_window as f64 * 0.9) as usize;
            
            if total_tokens > threshold {
                // Return error that will be caught by caller
                let msg = format!(
                    "Context overflow during tool execution: {} tokens used, {} available. Use /compact to reduce context.",
                    total_tokens, ctx_window
                );
                return Err(ollama_rs::error::OllamaError::Other(msg));
            }
        }
        
        let resp = self
            .ollama
            .send_chat_messages(self.build_request())
            .await?;

        self.process_response(resp).await
    }

    /// Get the number of registered tools (for future use)
    #[allow(dead_code)]
    pub fn tool_count(&self) -> usize {
        self.tools.len()
    }
}