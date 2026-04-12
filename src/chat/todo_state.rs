//! Todo list state management
//!
//! Provides a simple todo list for tracking tasks during a chat session.
//! The model can use todo tools to manage tasks explicitly, reducing the
//! need to search through conversation history.
//!
//! Each task has a status (pending/in_progress/done), a priority
//! (low/medium/high/critical), and optional tags for grouping.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Status of a task
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Task not yet started
    #[default]
    Pending,
    /// Task currently being worked on
    InProgress,
    /// Task completed
    Done,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Pending => write!(f, "pending"),
            TaskStatus::InProgress => write!(f, "in_progress"),
            TaskStatus::Done => write!(f, "done"),
        }
    }
}

impl std::str::FromStr for TaskStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" | "todo" | "not_started" => Ok(TaskStatus::Pending),
            "in_progress" | "inprogress" | "started" | "doing" => Ok(TaskStatus::InProgress),
            "done" | "completed" | "finished" => Ok(TaskStatus::Done),
            _ => Err(format!(
                "Invalid task status: '{}'. Use: pending, in_progress, or done",
                s
            )),
        }
    }
}

/// Priority of a task
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    /// Low priority
    Low,
    /// Medium priority (default)
    #[default]
    Medium,
    /// High priority
    High,
    /// Critical priority
    Critical,
}

impl std::fmt::Display for Priority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Priority::Low => write!(f, "low"),
            Priority::Medium => write!(f, "medium"),
            Priority::High => write!(f, "high"),
            Priority::Critical => write!(f, "critical"),
        }
    }
}

impl std::str::FromStr for Priority {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "low" | "l" => Ok(Priority::Low),
            "medium" | "med" | "m" | "normal" => Ok(Priority::Medium),
            "high" | "h" => Ok(Priority::High),
            "critical" | "c" | "urgent" => Ok(Priority::Critical),
            _ => Err(format!(
                "Invalid priority: '{}'. Use: low, medium, high, or critical",
                s
            )),
        }
    }
}

impl Priority {
    /// Get display symbol for priority
    #[allow(dead_code)]
    pub fn symbol(&self) -> &'static str {
        match self {
            Priority::Low => "🔵",
            Priority::Medium => "⚪",
            Priority::High => "🟡",
            Priority::Critical => "🔴",
        }
    }
}

/// Default function for serde deserialization of Priority
fn default_priority() -> Priority {
    Priority::Medium
}

/// A single task in the todo list
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Unique task identifier (1-indexed for user friendliness)
    #[serde(default)]
    pub id: usize,
    /// Task description
    #[serde(default)]
    pub description: String,
    /// Current status
    #[serde(default)]
    pub status: TaskStatus,
    /// Priority level
    #[serde(default = "default_priority")]
    pub priority: Priority,
    /// Tags for grouping (e.g., "bug", "feature")
    #[serde(default)]
    pub tags: Vec<String>,
    /// When the task was created
    #[serde(default)]
    pub created_at: DateTime<Utc>,
    /// When the task was last updated
    #[serde(default)]
    pub updated_at: DateTime<Utc>,
}

impl Task {
    /// Create a new task with the given id and description
    #[allow(dead_code)]
    pub fn new(id: usize, description: String) -> Self {
        let now = Utc::now();
        Self {
            id,
            description,
            status: TaskStatus::Pending,
            priority: Priority::Medium,
            tags: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Create a new task with priority and tags
    pub fn new_with_options(
        id: usize,
        description: String,
        priority: Priority,
        tags: Vec<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id,
            description,
            status: TaskStatus::Pending,
            priority,
            tags,
            created_at: now,
            updated_at: now,
        }
    }

    /// Get display symbol for the task status
    pub fn status_symbol(&self) -> &'static str {
        match self.status {
            TaskStatus::Pending => "☐",
            TaskStatus::InProgress => "►",
            TaskStatus::Done => "✓",
        }
    }

    /// Format tags for display (e.g., "#bug #feature")
    pub fn format_tags(&self) -> String {
        if self.tags.is_empty() {
            return String::new();
        }
        self.tags
            .iter()
            .map(|t| format!("#{}", t))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Filter criteria for listing tasks
#[derive(Debug, Clone, Default)]
pub struct TaskFilter {
    /// Filter by status
    pub status: Option<TaskStatus>,
    /// Filter by priority
    pub priority: Option<Priority>,
    /// Filter by tag (matches if task has this tag)
    pub tag: Option<String>,
}

/// Todo list state for a chat session
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TodoState {
    /// List of tasks (can be empty)
    #[serde(default)]
    pub tasks: Vec<Task>,
    /// Counter for generating task IDs
    #[serde(default)]
    next_id: usize,
}

impl TodoState {
    /// Create a new empty todo state
    pub fn new() -> Self {
        Self {
            tasks: Vec::new(),
            next_id: 1,
        }
    }

    /// Add a new task and return its ID
    #[allow(dead_code)]
    pub fn add(&mut self, description: String) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.tasks.push(Task::new(id, description));
        id
    }

    /// Add a new task with priority and tags, returning its ID
    pub fn add_with_options(
        &mut self,
        description: String,
        priority: Priority,
        tags: Vec<String>,
    ) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.tasks
            .push(Task::new_with_options(id, description, priority, tags));
        id
    }

    /// Get a task by ID
    pub fn get(&self, id: usize) -> Option<&Task> {
        self.tasks.iter().find(|t| t.id == id)
    }

    /// Get a mutable reference to a task by ID
    #[allow(dead_code)]
    pub fn get_mut(&mut self, id: usize) -> Option<&mut Task> {
        self.tasks.iter_mut().find(|t| t.id == id)
    }

    /// Update a task's status by ID
    /// Returns Ok(()) if found, Err(message) if not
    pub fn update_status(&mut self, id: usize, status: TaskStatus) -> Result<(), String> {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) {
            task.status = status;
            task.updated_at = Utc::now();
            Ok(())
        } else {
            Err(format!("Task {} not found", id))
        }
    }

    /// Edit a task's description, priority, and/or tags
    /// At least one field must be provided (Some value or non-empty).
    /// Returns Ok(()) if found, Err(message) if not.
    pub fn edit(
        &mut self,
        id: usize,
        description: Option<String>,
        priority: Option<Priority>,
        tags: Option<Vec<String>>,
    ) -> Result<(), String> {
        if description.is_none() && priority.is_none() && tags.is_none() {
            return Err(
                "Provide at least one field to update: description, priority, or tags".to_string(),
            );
        }

        let task = self
            .tasks
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or_else(|| format!("Task {} not found", id))?;

        if let Some(desc) = description {
            task.description = desc;
        }
        if let Some(pri) = priority {
            task.priority = pri;
        }
        if let Some(t) = tags {
            task.tags = t;
        }
        task.updated_at = Utc::now();
        Ok(())
    }

    /// Delete a task by ID
    /// Returns Ok(()) if found, Err(message) if not
    pub fn delete(&mut self, id: usize) -> Result<(), String> {
        let original_len = self.tasks.len();
        self.tasks.retain(|t| t.id != id);
        if self.tasks.len() < original_len {
            Ok(())
        } else {
            Err(format!("Task {} not found", id))
        }
    }

    /// Remove completed tasks
    /// Returns the number of tasks removed
    pub fn clear_done(&mut self) -> usize {
        let original_len = self.tasks.len();
        self.tasks.retain(|t| t.status != TaskStatus::Done);
        original_len - self.tasks.len()
    }

    /// Clear all tasks
    /// Returns the number of tasks cleared
    pub fn clear_all(&mut self) -> usize {
        let count = self.tasks.len();
        self.tasks.clear();
        self.next_id = 1;
        count
    }

    /// Count tasks by status
    pub fn count_by_status(&self, status: TaskStatus) -> usize {
        self.tasks.iter().filter(|t| t.status == status).count()
    }

    /// Count tasks by priority
    #[allow(dead_code)]
    pub fn count_by_priority(&self, priority: Priority) -> usize {
        self.tasks.iter().filter(|t| t.priority == priority).count()
    }

    /// Count total tasks
    #[allow(dead_code)]
    pub fn count(&self) -> usize {
        self.tasks.len()
    }

    /// Check if the list is empty
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    /// Format the todo list for display (unfiltered)
    #[allow(dead_code)]
    pub fn format_list(&self) -> String {
        self.format_list_filtered(&TaskFilter::default())
    }

    /// Format the todo list for display with optional filter
    pub fn format_list_filtered(&self, filter: &TaskFilter) -> String {
        let filtered: Vec<&Task> = self
            .tasks
            .iter()
            .filter(|t| {
                if let Some(status) = &filter.status
                    && t.status != *status
                {
                    return false;
                }
                if let Some(priority) = &filter.priority
                    && t.priority != *priority
                {
                    return false;
                }
                if let Some(tag) = &filter.tag
                    && !t.tags.iter().any(|t_tag| t_tag.eq_ignore_ascii_case(tag))
                {
                    return false;
                }
                true
            })
            .collect();

        if self.tasks.is_empty() {
            return "No tasks in the list.".to_string();
        }

        if filtered.is_empty() {
            return "No tasks match the filter criteria.".to_string();
        }

        let mut output = String::new();
        output.push_str("### TODO LIST\n\n");

        for task in &filtered {
            let tags_str = if task.tags.is_empty() {
                String::new()
            } else {
                format!(" {}", task.format_tags())
            };
            output.push_str(&format!(
                "{} {}. {} [{}] [{}]{tags}\n",
                task.status_symbol(),
                task.id,
                task.description,
                task.status,
                task.priority,
                tags = tags_str,
            ));
        }

        // Stats summary
        let pending = self.count_by_status(TaskStatus::Pending);
        let in_progress = self.count_by_status(TaskStatus::InProgress);
        let done = self.count_by_status(TaskStatus::Done);
        output.push_str(&format!(
            "\nStats: {} pending, {} in progress, {} done",
            pending, in_progress, done
        ));

        output
    }

    /// Convert TodoState to database rows
    pub fn to_rows(&self) -> Vec<crate::db::TodoRow> {
        self.tasks
            .iter()
            .map(|task| crate::db::TodoRow {
                task_id: task.id,
                description: task.description.clone(),
                status: task.status.to_string(),
                priority: task.priority.to_string(),
                tags: task.tags.join(","),
                created_at: task.created_at,
            })
            .collect()
    }

    /// Convert database rows to TodoState
    pub fn from_rows(rows: &[crate::db::TodoRow]) -> Self {
        let tasks: Vec<Task> = rows
            .iter()
            .map(|row| {
                let status = row.status.parse().unwrap_or(TaskStatus::Pending);
                let priority = row.priority.parse().unwrap_or(Priority::Medium);
                let tags = if row.tags.is_empty() {
                    Vec::new()
                } else {
                    row.tags
                        .split(',')
                        .map(|s| s.trim().to_lowercase())
                        .filter(|s| !s.is_empty())
                        .collect()
                };
                Task {
                    id: row.task_id,
                    description: row.description.clone(),
                    status,
                    priority,
                    tags,
                    created_at: row.created_at,
                    updated_at: row.created_at,
                }
            })
            .collect();

        let next_id = tasks
            .iter()
            .map(|t| t.id)
            .max()
            .map(|id| id + 1)
            .unwrap_or(1);

        TodoState { tasks, next_id }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_task_status_display() {
        assert_eq!(TaskStatus::Pending.to_string(), "pending");
        assert_eq!(TaskStatus::InProgress.to_string(), "in_progress");
        assert_eq!(TaskStatus::Done.to_string(), "done");
    }

    #[test]
    fn test_task_status_from_str() {
        assert!(matches!(
            TaskStatus::from_str("pending"),
            Ok(TaskStatus::Pending)
        ));
        assert!(matches!(
            TaskStatus::from_str("todo"),
            Ok(TaskStatus::Pending)
        ));
        assert!(matches!(
            TaskStatus::from_str("in_progress"),
            Ok(TaskStatus::InProgress)
        ));
        assert!(matches!(
            TaskStatus::from_str("doing"),
            Ok(TaskStatus::InProgress)
        ));
        assert!(matches!(TaskStatus::from_str("done"), Ok(TaskStatus::Done)));
        assert!(matches!(
            TaskStatus::from_str("completed"),
            Ok(TaskStatus::Done)
        ));
        assert!(TaskStatus::from_str("invalid").is_err());
    }

    #[test]
    fn test_priority_display() {
        assert_eq!(Priority::Low.to_string(), "low");
        assert_eq!(Priority::Medium.to_string(), "medium");
        assert_eq!(Priority::High.to_string(), "high");
        assert_eq!(Priority::Critical.to_string(), "critical");
    }

    #[test]
    fn test_priority_from_str() {
        assert!(matches!(Priority::from_str("low"), Ok(Priority::Low)));
        assert!(matches!(Priority::from_str("l"), Ok(Priority::Low)));
        assert!(matches!(Priority::from_str("medium"), Ok(Priority::Medium)));
        assert!(matches!(Priority::from_str("med"), Ok(Priority::Medium)));
        assert!(matches!(Priority::from_str("normal"), Ok(Priority::Medium)));
        assert!(matches!(Priority::from_str("high"), Ok(Priority::High)));
        assert!(matches!(Priority::from_str("h"), Ok(Priority::High)));
        assert!(matches!(
            Priority::from_str("critical"),
            Ok(Priority::Critical)
        ));
        assert!(matches!(
            Priority::from_str("urgent"),
            Ok(Priority::Critical)
        ));
        assert!(Priority::from_str("invalid").is_err());
    }

    #[test]
    fn test_priority_symbols() {
        assert_eq!(Priority::Low.symbol(), "🔵");
        assert_eq!(Priority::Medium.symbol(), "⚪");
        assert_eq!(Priority::High.symbol(), "🟡");
        assert_eq!(Priority::Critical.symbol(), "🔴");
    }

    #[test]
    fn test_task_new() {
        let task = Task::new(1, "Test task".to_string());
        assert_eq!(task.id, 1);
        assert_eq!(task.description, "Test task");
        assert!(matches!(task.status, TaskStatus::Pending));
        assert!(matches!(task.priority, Priority::Medium));
        assert!(task.tags.is_empty());
    }

    #[test]
    fn test_task_new_with_options() {
        let task = Task::new_with_options(
            1,
            "Bug fix".to_string(),
            Priority::High,
            vec!["bug".to_string(), "urgent".to_string()],
        );
        assert_eq!(task.id, 1);
        assert_eq!(task.description, "Bug fix");
        assert!(matches!(task.priority, Priority::High));
        assert_eq!(task.tags, vec!["bug", "urgent"]);
    }

    #[test]
    fn test_task_status_symbol() {
        let task = Task::new(1, "Test".to_string());
        assert_eq!(task.status_symbol(), "☐");

        let mut task = task;
        task.status = TaskStatus::InProgress;
        assert_eq!(task.status_symbol(), "►");

        task.status = TaskStatus::Done;
        assert_eq!(task.status_symbol(), "✓");
    }

    #[test]
    fn test_task_format_tags() {
        let mut task = Task::new(1, "Test".to_string());
        assert!(task.format_tags().is_empty());

        task.tags = vec!["bug".to_string(), "feature".to_string()];
        assert_eq!(task.format_tags(), "#bug #feature");
    }

    #[test]
    fn test_todo_state_add() {
        let mut state = TodoState::new();
        let id1 = state.add("Task 1".to_string());
        let id2 = state.add("Task 2".to_string());

        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(state.count(), 2);
    }

    #[test]
    fn test_todo_state_add_with_options() {
        let mut state = TodoState::new();
        let id = state.add_with_options(
            "Bug fix".to_string(),
            Priority::High,
            vec!["bug".to_string()],
        );
        assert_eq!(id, 1);
        assert_eq!(state.get(1).unwrap().priority, Priority::High);
        assert_eq!(state.get(1).unwrap().tags, vec!["bug"]);
    }

    #[test]
    fn test_todo_state_update_status() {
        let mut state = TodoState::new();
        state.add("Task 1".to_string());

        assert!(state.update_status(1, TaskStatus::InProgress).is_ok());
        assert_eq!(state.get(1).unwrap().status, TaskStatus::InProgress);

        assert!(state.update_status(999, TaskStatus::Done).is_err());
    }

    #[test]
    fn test_todo_state_edit() {
        let mut state = TodoState::new();
        state.add("Task 1".to_string());

        // Edit description only
        assert!(
            state
                .edit(1, Some("Updated task".to_string()), None, None)
                .is_ok()
        );
        assert_eq!(state.get(1).unwrap().description, "Updated task");

        // Edit priority only
        assert!(state.edit(1, None, Some(Priority::High), None).is_ok());
        assert_eq!(state.get(1).unwrap().priority, Priority::High);

        // Edit tags only
        assert!(
            state
                .edit(1, None, None, Some(vec!["feature".to_string()]))
                .is_ok()
        );
        assert_eq!(state.get(1).unwrap().tags, vec!["feature"]);

        // Edit with no fields
        assert!(state.edit(1, None, None, None).is_err());

        // Edit non-existent task
        assert!(state.edit(999, Some("x".to_string()), None, None).is_err());
    }

    #[test]
    fn test_todo_state_delete() {
        let mut state = TodoState::new();
        state.add("Task 1".to_string());
        state.add("Task 2".to_string());

        assert!(state.delete(1).is_ok());
        assert_eq!(state.count(), 1);
        assert!(state.get(1).is_none());
        assert!(state.get(2).is_some());

        // Delete non-existent task
        assert!(state.delete(999).is_err());
    }

    #[test]
    fn test_todo_state_clear_done() {
        let mut state = TodoState::new();
        state.add("Task 1".to_string());
        state.add("Task 2".to_string());
        state.add("Task 3".to_string());

        state.update_status(1, TaskStatus::Done).unwrap();
        state.update_status(2, TaskStatus::Done).unwrap();

        let removed = state.clear_done();
        assert_eq!(removed, 2);
        assert_eq!(state.count(), 1);
    }

    #[test]
    fn test_todo_state_clear_all() {
        let mut state = TodoState::new();
        state.add("Task 1".to_string());
        state.add("Task 2".to_string());

        let removed = state.clear_all();
        assert_eq!(removed, 2);
        assert!(state.is_empty());
    }

    #[test]
    fn test_todo_state_count_by_status() {
        let mut state = TodoState::new();
        state.add("Task 1".to_string());
        state.add("Task 2".to_string());
        state.add("Task 3".to_string());

        state.update_status(1, TaskStatus::InProgress).unwrap();
        state.update_status(2, TaskStatus::Done).unwrap();

        assert_eq!(state.count_by_status(TaskStatus::Pending), 1);
        assert_eq!(state.count_by_status(TaskStatus::InProgress), 1);
        assert_eq!(state.count_by_status(TaskStatus::Done), 1);
    }

    #[test]
    fn test_todo_state_count_by_priority() {
        let mut state = TodoState::new();
        let id1 = state.add_with_options("Task 1".to_string(), Priority::High, vec![]);
        let _id2 = state.add_with_options("Task 2".to_string(), Priority::Low, vec![]);
        let _id3 = state.add_with_options("Task 3".to_string(), Priority::High, vec![]);

        assert_eq!(state.count_by_priority(Priority::High), 2);
        assert_eq!(state.count_by_priority(Priority::Low), 1);
        assert_eq!(state.count_by_priority(Priority::Medium), 0);

        // Avoid unused variable warning
        assert_ne!(id1, 0);
    }

    #[test]
    fn test_todo_state_format_list() {
        let mut state = TodoState::new();
        state.add("Task 1".to_string());
        state.update_status(1, TaskStatus::Done).unwrap();

        let formatted = state.format_list();
        assert!(formatted.contains("✓"));
        assert!(formatted.contains("Task 1"));
        assert!(formatted.contains("Stats: 0 pending, 0 in progress, 1 done"));
    }

    #[test]
    fn test_todo_state_format_list_with_filter() {
        let mut state = TodoState::new();
        state.add_with_options(
            "Bug fix".to_string(),
            Priority::High,
            vec!["bug".to_string()],
        );
        state.add_with_options(
            "Feature".to_string(),
            Priority::Medium,
            vec!["feature".to_string()],
        );
        state.add("Regular task".to_string());

        // Filter by priority
        let filter = TaskFilter {
            priority: Some(Priority::High),
            ..Default::default()
        };
        let formatted = state.format_list_filtered(&filter);
        assert!(formatted.contains("Bug fix"));
        assert!(!formatted.contains("Feature"));
        assert!(!formatted.contains("Regular task"));

        // Filter by tag
        let filter = TaskFilter {
            tag: Some("bug".to_string()),
            ..Default::default()
        };
        let formatted = state.format_list_filtered(&filter);
        assert!(formatted.contains("Bug fix"));
        assert!(!formatted.contains("Feature"));
    }

    #[test]
    fn test_todo_state_serialization() {
        let mut state = TodoState::new();
        state.add("Task 1".to_string());
        state.update_status(1, TaskStatus::InProgress).unwrap();

        let json = serde_json::to_string(&state).unwrap();
        let deserialized: TodoState = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.count(), 1);
        assert_eq!(deserialized.get(1).unwrap().status, TaskStatus::InProgress);
    }

    #[test]
    fn test_todo_state_deserialization_backward_compat() {
        // Test that old JSON without priority/tags fields can still deserialize
        let old_json = r#"{"tasks":[{"id":1,"description":"Test","status":"pending","created_at":"2024-01-01T00:00:00Z","updated_at":"2024-01-01T00:00:00Z"}],"next_id":2}"#;
        let deserialized: TodoState = serde_json::from_str(old_json).unwrap();

        assert_eq!(deserialized.count(), 1);
        assert_eq!(deserialized.get(1).unwrap().description, "Test");
        assert_eq!(deserialized.get(1).unwrap().priority, Priority::Medium);
        assert!(deserialized.get(1).unwrap().tags.is_empty());
    }
}
