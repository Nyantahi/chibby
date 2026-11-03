use crate::engine::inbox;
use crate::engine::models::NotificationEntry;

/// List inbox entries, newest first.
#[tauri::command]
pub fn list_notifications() -> Result<Vec<NotificationEntry>, String> {
    inbox::list().map_err(|e| e.to_string())
}

/// Mark one inbox entry as read.
#[tauri::command]
pub fn mark_notification_read(id: String) -> Result<(), String> {
    inbox::mark_read(&id).map_err(|e| e.to_string())
}

/// Mark every inbox entry as read.
#[tauri::command]
pub fn mark_all_notifications_read() -> Result<(), String> {
    inbox::mark_all_read().map_err(|e| e.to_string())
}

/// Mark the entries about one run as read.
#[tauri::command]
pub fn mark_run_read(run_id: String) -> Result<(), String> {
    inbox::mark_run_read(&run_id).map_err(|e| e.to_string())
}

/// Remove one inbox entry.
#[tauri::command]
pub fn delete_notification(id: String) -> Result<(), String> {
    inbox::delete(&id).map_err(|e| e.to_string())
}
