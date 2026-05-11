//! Todo list management tools
//!
//! These tools allow the LLM to track tasks explicitly during a conversation,
//! reducing the need to search through conversation history.
//!
//! Tasks support status (pending/in_progress/done), priority (low/medium/high/critical),
//! and tags for grouping (e.g., "bug", "feature").

use std::sync::{Arc, Mutex};

use once_cell::sync::OnceCell;

use crate::chat::todo_state::{Priority, TaskFilter, TaskStatus, TodoState};
use crate::debug_tools::{log_tool_call, log_tool_result};

/// Global todo state shared between tools
static TODO_STATE: OnceCell<Arc<Mutex<TodoState>>> = OnceCell::new();

/// Get or initialize the global todo state
pub fn get_todo_state() -> Arc<Mutex<TodoState>> {
    TODO_STATE
        .get_or_init(|| Arc::new(Mutex::new(TodoState::new())))
        .clone()
}

/// Load todos from a session into the global state.
///
/// Call this at the start of the REPL to restore the session's todo list.
pub fn load_from_session(session_todos: &TodoState) {
    let state = get_todo_state();
    #[expect(clippy::expect_used)] // mutex poisoning indicates a programming bug
    let mut guard = state.lock().expect("lock poisoned: todo state");
    *guard = session_todos.clone();
}

/// Save the global todo state to a session.
///
/// Call this before persisting the session to save any changes made by tools.
pub fn save_to_session() -> TodoState {
    let state = get_todo_state();
    #[expect(clippy::expect_used)] // mutex poisoning indicates a programming bug
    let guard = state.lock().expect("lock poisoned: todo state");
    guard.clone()
}

/// Format the current todo list for display in the system prompt.
///
/// Returns None if the list is empty, otherwise returns the formatted string.
pub fn format_todos_for_prompt() -> Option<String> {
    let state = get_todo_state();
    #[expect(clippy::expect_used)] // mutex poisoning indicates a programming bug
    let guard = state.lock().expect("lock poisoned: todo state");

    if guard.tasks.is_empty() {
        return None;
    }

    let mut output = String::new();
    output.push_str("### ACTIVE TASKS\n\n");
    output.push_str("You have tasks to track. Use todo tools to manage them:\n\n");

    for task in &guard.tasks {
        let status_icon = match task.status {
            TaskStatus::Pending => "☐",
            TaskStatus::InProgress => "►",
            TaskStatus::Done => "✓",
        };
        let tags_str = if task.tags.is_empty() {
            String::new()
        } else {
            format!(" {}", task.format_tags())
        };
        output.push_str(&format!(
            "{} {} - {} [{}][{}]{}\n",
            status_icon, task.id, task.description, task.status, task.priority, tags_str
        ));
    }

    output.push_str("\nUse `todo_list()` to see the full list with descriptions.\n");

    Some(output)
}

/// Parse task ID from string, returning helpful error message on failure.
fn parse_task_id(task_id: &str) -> Result<usize, String> {
    task_id.parse::<usize>().map_err(|_| {
        format!(
            "Error: Invalid task ID '{}'. Must be a number like '1', '2', etc.",
            task_id
        )
    })
}

/// Parse priority from optional string, defaulting to Medium.
fn parse_priority(priority: Option<String>) -> Priority {
    priority
        .as_deref()
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse().ok())
        .unwrap_or(Priority::Medium)
}

/// Parse tags from optional comma-separated string.
fn parse_tags(tags: Option<String>) -> Vec<String> {
    tags.as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.split(',')
                .map(|t| t.trim().to_lowercase())
                .filter(|t| !t.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// Add a new task to the todo list.
///
/// Use this tool when starting a new task that needs to be tracked.
/// The task will be assigned a unique ID and start with "pending" status.
///
/// # Arguments
/// * `description` - A clear, concise description of the task to track.
///   - Example: "Implement user authentication"
///   - Example: "Fix the bug in file parsing"
/// * `priority` - Optional priority level. One of: "low", "medium", "high", "critical".
///   - Default: "medium"
///   - Aliases: "l"=low, "m"=medium, "h"=high, "c"=critical, "urgent"=critical
/// * `tags` - Optional comma-separated tags for grouping. Use lowercase.
///   - Example: "bug,urgent"
///   - Example: "feature,frontend"
///
/// # Returns
/// Confirmation message with the task ID, status, priority, and tags.
///
/// # Example
/// ```ignore
/// todo_add("Fix login bug".to_string(), Some("high".to_string()), Some("bug,auth".to_string()))
/// // Returns: "Added task 1: Fix login bug [pending] [high] #bug #auth"
/// ```
#[ollama_rs::function]
pub async fn todo_add(
    description: String,
    priority: Option<String>,
    tags: Option<String>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let priority_val = parse_priority(priority);
    let tags_val = parse_tags(tags);

    log_tool_call(
        "todo_add",
        &[
            ("description".to_string(), description.clone()),
            ("priority".to_string(), priority_val.to_string()),
            (
                "tags".to_string(),
                if tags_val.is_empty() {
                    "none".to_string()
                } else {
                    tags_val.join(",")
                },
            ),
        ],
    );

    let state = get_todo_state();
    #[expect(clippy::expect_used)] // mutex poisoning indicates a programming bug
    let mut guard = state.lock().expect("lock poisoned: todo state");
    let id = guard.add_with_options(description.clone(), priority_val, tags_val.clone());

    let mut result = format!(
        "Added task {}: {} [pending] [{}]",
        id, description, priority_val
    );
    if !tags_val.is_empty() {
        result.push_str(&format!(
            " {}",
            tags_val
                .iter()
                .map(|t| format!("#{}", t))
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }

    log_tool_result("todo_add", &result);
    Ok(result)
}

/// Update the status of an existing task.
///
/// Use this tool to mark tasks as in progress or completed.
///
/// # Arguments
/// * `task_id` - The ID of the task to update (from todo_add or todo_list).
///   - Example: "1", "2", "3"
/// * `status` - The new status. Must be one of:
///   - "pending" or "todo" or "not_started" - Task not started
///   - "in_progress" or "inprogress" or "started" or "doing" - Task being worked on
///   - "done" or "completed" or "finished" - Task completed
///
/// # Returns
/// Confirmation message or error if task not found.
///
/// # Example
/// ```ignore
/// todo_update("1".to_string(), "in_progress".to_string())
/// // Returns: "Task 1 marked as in_progress"
/// ```
#[ollama_rs::function]
pub async fn todo_update(
    task_id: String,
    status: String,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call(
        "todo_update",
        &[
            ("task_id".to_string(), task_id.clone()),
            ("status".to_string(), status.clone()),
        ],
    );

    let id: usize = match parse_task_id(&task_id) {
        Ok(id) => id,
        Err(e) => {
            log_tool_result("todo_update", &e);
            return Ok(e);
        }
    };

    let new_status: TaskStatus = match status.parse() {
        Ok(s) => s,
        Err(e) => {
            let err = format!("Error: {}", e);
            log_tool_result("todo_update", &err);
            return Ok(err);
        }
    };

    let state = get_todo_state();
    #[expect(clippy::expect_used)] // mutex poisoning indicates a programming bug
    let mut guard = state.lock().expect("lock poisoned: todo state");

    let result = match guard.update_status(id, new_status) {
        Ok(()) => format!("Task {} marked as {}", id, new_status),
        Err(e) => format!("Error: {}", e),
    };

    log_tool_result("todo_update", &result);
    Ok(result)
}

/// Get details of a single task by ID.
///
/// Use this tool to retrieve the full details of a specific task,
/// including its description, status, priority, and tags.
///
/// # Arguments
/// * `task_id` - The ID of the task to retrieve.
///   - Example: "1", "2", "3"
///
/// # Returns
/// Detailed task information or error if task not found.
///
/// # Example
/// ```ignore
/// todo_get("1".to_string())
/// // Returns: "Task 1: Fix login bug\nStatus: pending\nPriority: high\nTags: #bug #auth"
/// ```
#[ollama_rs::function]
pub async fn todo_get(task_id: String) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call("todo_get", &[("task_id".to_string(), task_id.clone())]);

    let id: usize = match parse_task_id(&task_id) {
        Ok(id) => id,
        Err(e) => {
            log_tool_result("todo_get", &e);
            return Ok(e);
        }
    };

    let state = get_todo_state();
    #[expect(clippy::expect_used)] // mutex poisoning indicates a programming bug
    let guard = state.lock().expect("lock poisoned: todo state");

    let result = match guard.get(id) {
        Some(task) => {
            let mut output = format!(
                "Task {}: {}\nStatus: {}\nPriority: {}",
                task.id, task.description, task.status, task.priority
            );
            if !task.tags.is_empty() {
                output.push_str(&format!(
                    "\nTags: {}",
                    task.tags
                        .iter()
                        .map(|t| format!("#{}", t))
                        .collect::<Vec<_>>()
                        .join(" ")
                ));
            }
            output
        }
        None => format!("Error: Task {} not found", id),
    };

    log_tool_result("todo_get", &result);
    Ok(result)
}

/// Edit a task's description, priority, and/or tags.
///
/// Use this to correct or update a task's metadata.
/// At least one of `description`, `priority`, or `tags` must be provided.
///
/// # Arguments
/// * `task_id` - The ID of the task to edit.
///   - Example: "1", "2", "3"
/// * `description` - New description for the task. Optional.
///   - Example: "Updated task description"
/// * `priority` - New priority level. Optional. One of: "low", "medium", "high", "critical".
/// * `tags` - New comma-separated tags. Optional. Replaces all existing tags.
///   - Example: "bug,urgent"
///
/// # Returns
/// Confirmation message showing what was updated, or error if task not found.
///
/// # Example
/// ```ignore
/// // Change priority only
/// todo_edit("1".to_string(), None, Some("high".to_string()), None)
///
/// // Change tags only
/// todo_edit("1".to_string(), None, None, Some("bug,frontend".to_string()))
/// ```
#[ollama_rs::function]
pub async fn todo_edit(
    task_id: String,
    description: Option<String>,
    priority: Option<String>,
    tags: Option<String>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // Normalize empty strings to None
    let description = description.filter(|s| !s.is_empty());
    let priority = priority.filter(|s| !s.is_empty());
    let tags = tags.filter(|s| !s.is_empty());

    log_tool_call(
        "todo_edit",
        &[
            ("task_id".to_string(), task_id.clone()),
            (
                "description".to_string(),
                description.as_deref().unwrap_or("unchanged").to_string(),
            ),
            (
                "priority".to_string(),
                priority.as_deref().unwrap_or("unchanged").to_string(),
            ),
            (
                "tags".to_string(),
                tags.as_deref().unwrap_or("unchanged").to_string(),
            ),
        ],
    );

    let id: usize = match parse_task_id(&task_id) {
        Ok(id) => id,
        Err(e) => {
            log_tool_result("todo_edit", &e);
            return Ok(e);
        }
    };

    let priority_val: Option<Priority> = priority.and_then(|s| s.parse().ok());
    let tags_val: Option<Vec<String>> = tags.map(|s| {
        s.split(',')
            .map(|t| t.trim().to_lowercase())
            .filter(|t| !t.is_empty())
            .collect()
    });

    let state = get_todo_state();
    #[expect(clippy::expect_used)] // mutex poisoning indicates a programming bug
    let mut guard = state.lock().expect("lock poisoned: todo state");

    let result = match guard.edit(id, description, priority_val, tags_val) {
        Ok(()) => {
            #[expect(clippy::expect_used)] // task just edited successfully, guaranteed to exist
            let task = guard.get(id).expect("task just edited successfully");
            let mut msg = format!("Task {} updated:", id);
            msg.push_str(&format!("\n  Description: {}", task.description));
            msg.push_str(&format!("\n  Status: {}", task.status));
            msg.push_str(&format!("\n  Priority: {}", task.priority));
            if !task.tags.is_empty() {
                msg.push_str(&format!(
                    "\n  Tags: {}",
                    task.tags
                        .iter()
                        .map(|t| format!("#{}", t))
                        .collect::<Vec<_>>()
                        .join(" ")
                ));
            }
            msg
        }
        Err(e) => format!("Error: {}", e),
    };

    log_tool_result("todo_edit", &result);
    Ok(result)
}

/// Delete a specific task by ID.
///
/// Use this to remove a task that is no longer needed.
/// This permanently removes the task from the list.
///
/// # Arguments
/// * `task_id` - The ID of the task to delete.
///   - Example: "1", "2", "3"
///
/// # Returns
/// Confirmation message or error if task not found.
///
/// # Example
/// ```ignore
/// todo_delete("3".to_string())
/// // Returns: "Deleted task 3: Fix typo"
/// ```
#[ollama_rs::function]
pub async fn todo_delete(
    task_id: String,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call("todo_delete", &[("task_id".to_string(), task_id.clone())]);

    let id: usize = match parse_task_id(&task_id) {
        Ok(id) => id,
        Err(e) => {
            log_tool_result("todo_delete", &e);
            return Ok(e);
        }
    };

    let state = get_todo_state();
    #[expect(clippy::expect_used)] // mutex poisoning indicates a programming bug
    let mut guard = state.lock().expect("lock poisoned: todo state");

    // Get task description before deleting for a better message
    let task_desc = guard.get(id).map(|t| t.description.clone());

    let result = match guard.delete(id) {
        Ok(()) => {
            if let Some(desc) = task_desc {
                format!("Deleted task {}: {}", id, desc)
            } else {
                format!("Deleted task {}", id)
            }
        }
        Err(e) => format!("Error: {}", e),
    };

    log_tool_result("todo_delete", &result);
    Ok(result)
}

/// List tasks in the todo list, optionally filtered by status, priority, or tag.
///
/// Use this tool to see the current task list. Call this before editing tasks
/// to confirm the task ID and current state.
///
/// # Arguments
/// * `filter` - Optional filter criteria. One of:
///   - Status filter: "pending", "in_progress", "done"
///   - Priority filter: "low", "medium", "high", "critical"
///   - Tag filter: starts with "#" (e.g., "#bug", "#feature")
///   - Omit to show all tasks
///
/// # Returns
/// Formatted list of tasks matching the filter, with IDs, statuses, priorities, and tags.
///
/// # Example
/// ```ignore
/// todo_list(None)                    // Show all tasks
/// todo_list(Some("pending".to_string()))     // Show pending tasks
/// todo_list(Some("high".to_string()))         // Show high priority tasks
/// todo_list(Some("#bug".to_string()))          // Show tasks tagged "bug"
/// ```
#[ollama_rs::function]
pub async fn todo_list(
    filter: Option<String>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let filter_val = filter.filter(|s| !s.is_empty());

    log_tool_call(
        "todo_list",
        &[(
            "filter".to_string(),
            filter_val.as_deref().unwrap_or("all").to_string(),
        )],
    );

    let state = get_todo_state();
    #[expect(clippy::expect_used)] // mutex poisoning indicates a programming bug
    let guard = state.lock().expect("lock poisoned: todo state");

    let task_filter = if let Some(ref f) = filter_val {
        // Check for tag filter (starts with #)
        if let Some(tag) = f.strip_prefix('#') {
            TaskFilter {
                tag: Some(tag.to_lowercase()),
                ..Default::default()
            }
        }
        // Check for status filter
        else if let Ok(status) = f.parse::<TaskStatus>() {
            TaskFilter {
                status: Some(status),
                ..Default::default()
            }
        }
        // Check for priority filter
        else if let Ok(priority) = f.parse::<Priority>() {
            TaskFilter {
                priority: Some(priority),
                ..Default::default()
            }
        }
        // Unknown filter - try as tag without #
        else {
            TaskFilter {
                tag: Some(f.to_lowercase()),
                ..Default::default()
            }
        }
    } else {
        TaskFilter::default()
    };

    let result = guard.format_list_filtered(&task_filter);
    log_tool_result("todo_list", &result);
    Ok(result)
}

/// Clear completed (done) tasks from the list.
///
/// Use this tool to clean up the task list after tasks are finished.
/// Only removes tasks with "done" status. Pending and in-progress tasks remain.
///
/// # Returns
/// Number of tasks removed.
///
/// # Example
/// ```ignore
/// todo_clear_done()
/// // Returns: "Removed 3 completed tasks"
/// ```
#[ollama_rs::function]
pub async fn todo_clear_done() -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call("todo_clear_done", &[]);

    let state = get_todo_state();
    #[expect(clippy::expect_used)] // mutex poisoning indicates a programming bug
    let mut guard = state.lock().expect("lock poisoned: todo state");
    let removed = guard.clear_done();

    let result = if removed == 0 {
        "No completed tasks to remove.".to_string()
    } else if removed == 1 {
        "Removed 1 completed task.".to_string()
    } else {
        format!("Removed {} completed tasks.", removed)
    };

    log_tool_result("todo_clear_done", &result);
    Ok(result)
}

/// Clear all tasks from the list.
///
/// Use this tool to start fresh with an empty task list.
/// WARNING: This removes ALL tasks regardless of status.
///
/// # Returns
/// Number of tasks cleared.
///
/// # Example
/// ```ignore
/// todo_clear_all()
/// // Returns: "Cleared 5 tasks from the list"
/// ```
#[ollama_rs::function]
pub async fn todo_clear_all() -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    log_tool_call("todo_clear_all", &[]);

    let state = get_todo_state();
    #[expect(clippy::expect_used)] // mutex poisoning indicates a programming bug
    let mut guard = state.lock().expect("lock poisoned: todo state");
    let count = guard.clear_all();

    let result = if count == 0 {
        "The task list was already empty.".to_string()
    } else if count == 1 {
        "Cleared 1 task from the list.".to_string()
    } else {
        format!("Cleared {} tasks from the list.", count)
    };

    log_tool_result("todo_clear_all", &result);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::todo_state::TodoState;

    #[test]
    fn test_todo_add() {
        let mut state = TodoState::new();
        let id = state.add("Test task".to_string());

        assert_eq!(id, 1);
        assert_eq!(state.count(), 1);
    }

    #[test]
    fn test_todo_update() {
        let mut state = TodoState::new();
        state.add("Test task".to_string());

        let result = state.update_status(1, TaskStatus::InProgress);
        assert!(result.is_ok());
        assert_eq!(state.get(1).unwrap().status, TaskStatus::InProgress);
    }

    #[test]
    fn test_todo_clear_done() {
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
}
