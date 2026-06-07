// Error case: tool function must be documented
use sprachspiel_tool_derive::tool;

#[tool]
pub async fn no_docs(x: i32) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    Ok(format!("{}", x))
}
