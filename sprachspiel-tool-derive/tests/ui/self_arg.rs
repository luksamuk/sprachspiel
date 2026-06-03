// Error case: self argument is not allowed
use sprachspiel_tool_derive::tool;

pub struct Foo;

impl Foo {
    #[tool]
    pub async fn has_self(&self, x: i32) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        Ok(format!("{}", x))
    }
}
