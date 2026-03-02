//! Todo list state management
//!
//! Provides a simple todo list for tracking tasks during a chat session.
//! The model can use todo tools to manage tasks explicitly, reducing the
//! need to search through conversation history.

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

/// A single task in the todo list
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Unique task identifier (1-indexed for user friendliness)
    pub id: usize,
    /// Task description
    pub description: String,
    /// Current status
    pub status: TaskStatus,
    /// When the task was created
    pub created_at: DateTime<Utc>,
    /// When the task was last updated
    pub updated_at: DateTime<Utc>,
}

impl Task {
    /// Create a new task with the given id and description
    pub fn new(id: usize, description: String) -> Self {
        let now = Utc::now();
        Self {
            id,
            description,
            status: TaskStatus::Pending,
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
    pub fn add(&mut self, description: String) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.tasks.push(Task::new(id, description));
        id
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

    /// Get a task by ID (for testing/external use)
    #[allow(dead_code)]
    pub fn get(&self, id: usize) -> Option<&Task> {
        self.tasks.iter().find(|t| t.id == id)
    }

    /// Get all tasks (for testing/external use)
    #[allow(dead_code)]
    pub fn all(&self) -> &[Task] {
        &self.tasks
    }

    /// Count tasks by status
    pub fn count_by_status(&self, status: TaskStatus) -> usize {
        self.tasks.iter().filter(|t| t.status == status).count()
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

    /// Format the todo list for display
    pub fn format_list(&self) -> String {
        if self.tasks.is_empty() {
            return "No tasks in the list.".to_string();
        }

        let mut output = String::new();
        output.push_str("### TODO LIST\n\n");

        for task in &self.tasks {
            output.push_str(&format!(
                "{} {}. {} [{}]\n",
                task.status_symbol(),
                task.id,
                task.description,
                task.status
            ));
        }

        output.push_str(&format!(
            "\nStats: {} pending, {} in progress, {} done",
            self.count_by_status(TaskStatus::Pending),
            self.count_by_status(TaskStatus::InProgress),
            self.count_by_status(TaskStatus::Done)
        ));

        output
    }

    /// Format the todo list for system prompt inclusion
    /// Returns None if the list is empty
    pub fn format_for_prompt(&self) -> Option<String> {
        if self.tasks.is_empty() {
            return None;
        }

        let mut output = String::new();
        output.push_str("### CURRENT TASKS\n\n");
        output.push_str("You are tracking the following tasks:\n\n");

        for task in &self.tasks {
            output.push_str(&format!(
                "{} {} ({}): {}\n",
                task.status_symbol(),
                task.id,
                task.status,
                task.description
            ));
        }

        output.push_str("\n### END TASKS\n");
        Some(output)
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
    fn test_task_new() {
        let task = Task::new(1, "Test task".to_string());
        assert_eq!(task.id, 1);
        assert_eq!(task.description, "Test task");
        assert!(matches!(task.status, TaskStatus::Pending));
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
    fn test_todo_state_add() {
        let mut state = TodoState::new();
        let id1 = state.add("Task 1".to_string());
        let id2 = state.add("Task 2".to_string());

        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(state.count(), 2);
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
    fn test_todo_state_format_for_prompt() {
        let mut state = TodoState::new();

        // Empty state returns None
        assert!(state.format_for_prompt().is_none());

        // With tasks returns Some
        state.add("Task 1".to_string());
        state.update_status(1, TaskStatus::InProgress).unwrap();

        let prompt = state.format_for_prompt().unwrap();
        assert!(prompt.contains("CURRENT TASKS"));
        assert!(prompt.contains("► 1 (in_progress): Task 1"));
        assert!(prompt.contains("END TASKS"));
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
}
