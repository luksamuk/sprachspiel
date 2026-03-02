//! Todo list management tools
//!
//! These tools allow the LLM to track tasks explicitly during a conversation,
//! reducing the need to search through conversation history.

use std::sync::{Arc, Mutex};

use once_cell::sync::OnceCell;

use crate::chat::todo_state::TaskStatus;

/// Global todo state shared between tools
static TODO_STATE: OnceCell<Arc<Mutex<crate::chat::todo_state::TodoState>>> = OnceCell::new();

/// Get or initialize the global todo state
fn get_todo_state() -> Arc<Mutex<crate::chat::todo_state::TodoState>> {
    TODO_STATE
        .get_or_init(|| {
            Arc::new(Mutex::new(crate::chat::todo_state::TodoState::new()))
        })
        .clone()
}

/// Reset the todo state (for testing or new session)
#[cfg(test)]
pub fn reset_todo_state() {
    let state = get_todo_state();
    let mut guard = state.lock().unwrap();
    *guard = crate::chat::todo_state::TodoState::new();
}

/// Set the todo state from an existing state (for session persistence)
/// 
/// This function is called when loading a saved session to restore
/// the todo list state from the ChatSession.todos field.
#[allow(dead_code)]
pub fn set_todo_state(state: crate::chat::todo_state::TodoState) {
    let global_state = get_todo_state();
    let mut guard = global_state.lock().unwrap();
    *guard = state;
}

/// Get a copy of the current todo state (for session persistence)
///
/// This function is called when saving a session to persist
/// the current todo list state to the ChatSession.todos field.
#[allow(dead_code)]
pub fn get_todo_copy() -> crate::chat::todo_state::TodoState {
    let global_state = get_todo_state();
    let guard = global_state.lock().unwrap();
    guard.clone()
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
///
/// # Returns
/// Confirmation message with the task ID and current status, or an error message.
///
/// # Example
/// ```ignore
/// todo_add("Review the pull request".to_string())
/// // Returns: "Added task 1: Review the pull request [pending]"
/// ```
#[ollama_rs::function]
pub async fn todo_add(description: String) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let state = get_todo_state();
    let mut guard = state.lock().unwrap();
    let id = guard.add(description.clone());
    Ok(format!("Added task {}: {} [pending]", id, description))
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
pub async fn todo_update(task_id: String, status: String) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let id: usize = match task_id.parse() {
        Ok(id) => id,
        Err(_) => {
            let err = format!("Error: Invalid task ID '{}'. Must be a number like '1', '2', etc.", task_id);
            return Ok(err);
        }
    };
    
    let new_status: TaskStatus = match status.parse() {
        Ok(s) => s,
        Err(e) => return Ok(format!("Error: {}", e)),
    };
    
    let state = get_todo_state();
    let mut guard = state.lock().unwrap();
    
    match guard.update_status(id, new_status) {
        Ok(()) => Ok(format!("Task {} marked as {}", id, new_status)),
        Err(e) => Ok(format!("Error: {}", e)),
    }
}

/// List all tasks in the todo list.
///
/// Use this tool to see the current task list with their IDs and statuses.
/// Call this before updating tasks to see what exists.
///
/// # Returns
/// Formatted list of all tasks with IDs, statuses, and descriptions.
/// Shows task count summary at the end.
///
/// # Example
/// ```ignore
/// todo_list()
/// // Returns:
/// // "### TODO LIST
/// //  
/// //  ☐ 1. Review pull request [pending]
/// //  ► 2. Implement authentication [in_progress]
/// //  ✓ 3. Write tests [done]
/// //  
/// //  Stats: 1 pending, 1 in progress, 1 done"
/// ```
#[ollama_rs::function]
pub async fn todo_list() -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let state = get_todo_state();
    let guard = state.lock().unwrap();
    Ok(guard.format_list())
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
    let state = get_todo_state();
    let mut guard = state.lock().unwrap();
    let removed = guard.clear_done();
    
    if removed == 0 {
        Ok("No completed tasks to remove.".to_string())
    } else if removed == 1 {
        Ok("Removed 1 completed task.".to_string())
    } else {
        Ok(format!("Removed {} completed tasks.", removed))
    }
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
    let state = get_todo_state();
    let mut guard = state.lock().unwrap();
    let count = guard.clear_all();
    
    if count == 0 {
        Ok("The task list was already empty.".to_string())
    } else if count == 1 {
        Ok("Cleared 1 task from the list.".to_string())
    } else {
        Ok(format!("Cleared {} tasks from the list.", count))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_todo_add() {
        reset_todo_state();
        
        // This would be async in real usage, but we can test the state directly
        let state = get_todo_state();
        let mut guard = state.lock().unwrap();
        let id = guard.add("Test task".to_string());
        
        assert_eq!(id, 1);
        assert_eq!(guard.count(), 1);
    }

    #[test]
    fn test_todo_update() {
        reset_todo_state();
        
        let state = get_todo_state();
        {
            let mut guard = state.lock().unwrap();
            guard.add("Test task".to_string());
        }
        
        {
            let mut guard = state.lock().unwrap();
            let result = guard.update_status(1, TaskStatus::InProgress);
            assert!(result.is_ok());
        }
        
        let guard = state.lock().unwrap();
        assert_eq!(guard.get(1).unwrap().status, TaskStatus::InProgress);
    }

    #[test]
    fn test_todo_clear_done() {
        reset_todo_state();
        
        let state = get_todo_state();
        {
            let mut guard = state.lock().unwrap();
            guard.add("Task 1".to_string());
            guard.add("Task 2".to_string());
            guard.add("Task 3".to_string());
            guard.update_status(1, TaskStatus::Done).unwrap();
            guard.update_status(2, TaskStatus::Done).unwrap();
        }
        
        {
            let mut guard = state.lock().unwrap();
            let removed = guard.clear_done();
            assert_eq!(removed, 2);
            assert_eq!(guard.count(), 1);
        }
    }
}