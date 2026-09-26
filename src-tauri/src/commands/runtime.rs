use tauri::State;

use crate::storage::{RuntimeEnvironment, StoragePaths};

/// 返回编译模式和前端 Store 应使用的绝对路径。
#[tauri::command]
pub fn get_runtime_environment(storage: State<'_, StoragePaths>) -> RuntimeEnvironment {
    RuntimeEnvironment::from(storage.inner())
}
