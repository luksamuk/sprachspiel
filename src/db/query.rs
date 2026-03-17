//! Query builder utilities for dynamic SQL construction
//!
//! Provides `WhereBuilder` for constructing parameterized WHERE clauses
//! with optional conditions, eliminating duplicate SQL patterns.

use rusqlite::ToSql;

/// Helper for building dynamic WHERE clauses with parameterized queries.
///
/// # Example
///
/// ```ignore
/// use crate::db::WhereBuilder;
///
/// let mut builder = WhereBuilder::new();
/// builder
///     .add("content_type = 'note'")
///     .add_option("scope = ?", scope.map(|s| s.to_string()))
///     .add_option_str("project_id = ?", project_id);
///
/// let sql = format!("SELECT * FROM items {}", builder.build_where());
/// let params = builder.into_params();
/// conn.query_map(&sql, rusqlite::params_from_iter(params.iter()), |row| { ... })?;
/// ```
pub struct WhereBuilder {
    conditions: Vec<String>,
    params: Vec<Box<dyn ToSql>>,
}

impl WhereBuilder {
    /// Create a new empty builder
    pub fn new() -> Self {
        Self {
            conditions: Vec::new(),
            params: Vec::new(),
        }
    }

    /// Add a static condition (no parameter)
    pub fn add(&mut self, condition: impl Into<String>) -> &mut Self {
        self.conditions.push(condition.into());
        self
    }

    /// Add a condition with an optional typed parameter
    pub fn add_option<T: ToSql + 'static>(
        &mut self,
        condition: impl Into<String>,
        param: Option<T>,
    ) -> &mut Self {
        if let Some(p) = param {
            self.conditions.push(condition.into());
            self.params.push(Box::new(p));
        }
        self
    }

    /// Add a condition with an optional string parameter
    pub fn add_option_str(
        &mut self,
        condition: impl Into<String>,
        param: Option<&str>,
    ) -> &mut Self {
        if let Some(p) = param {
            self.conditions.push(condition.into());
            self.params.push(Box::new(p.to_string()));
        }
        self
    }

    /// Build WHERE clause (returns empty string if no conditions)
    pub fn build_where(&self) -> String {
        if self.conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", self.conditions.join(" AND "))
        }
    }

    /// Consume builder and return parameters
    pub fn into_params(self) -> Vec<Box<dyn ToSql>> {
        self.params
    }
}

impl Default for WhereBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_builder() {
        let builder = WhereBuilder::new();
        assert_eq!(builder.build_where(), "");
    }

    #[test]
    fn test_static_condition() {
        let mut builder = WhereBuilder::new();
        builder.add("content_type = 'note'");
        assert_eq!(builder.build_where(), "WHERE content_type = 'note'");
    }

    #[test]
    fn test_multiple_conditions() {
        let mut builder = WhereBuilder::new();
        builder
            .add("content_type = 'note'")
            .add_option("scope = ?", Some("project".to_string()));
        assert_eq!(
            builder.build_where(),
            "WHERE content_type = 'note' AND scope = ?"
        );
    }

    #[test]
    fn test_optional_condition_present() {
        let mut builder = WhereBuilder::new();
        builder.add_option("scope = ?", Some("project".to_string()));
        assert_eq!(builder.build_where(), "WHERE scope = ?");
    }

    #[test]
    fn test_optional_condition_absent() {
        let mut builder = WhereBuilder::new();
        builder.add_option::<String>("scope = ?", None);
        assert_eq!(builder.build_where(), "");
    }

    #[test]
    fn test_add_option_str() {
        let mut builder = WhereBuilder::new();
        builder.add_option_str("project_id = ?", Some("my-project"));
        assert_eq!(builder.build_where(), "WHERE project_id = ?");

        let mut builder2 = WhereBuilder::new();
        builder2.add_option_str("project_id = ?", None);
        assert_eq!(builder2.build_where(), "");
    }

    #[test]
    fn test_into_params() {
        let mut builder = WhereBuilder::new();
        builder
            .add_option("a = ?", Some(1))
            .add_option("b = ?", Some(2));

        let params = builder.into_params();
        assert_eq!(params.len(), 2);
    }
}
