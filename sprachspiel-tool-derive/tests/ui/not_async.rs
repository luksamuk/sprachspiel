// Error case: tool function must be async
use sprachspiel_tool_derive::tool;

#[tool]
pub fn not_async(x: i32) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    Ok(format!("{}", x))
}
