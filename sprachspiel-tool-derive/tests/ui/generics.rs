// Error case: generics are not allowed
use sprachspiel_tool_derive::tool;

#[tool]
pub async fn has_generics<T>(x: T) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    Ok(format!("{:?}", x))
}
