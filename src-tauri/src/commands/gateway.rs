//! 诊断日志读取/清空 command(小说板块删除后,gateway 域仅存诊断)

use tauri::AppHandle;

/// 读取诊断日志(应用内查看)
#[tauri::command]
pub(crate) fn diag_read(app: AppHandle) -> Result<String, String> {
    use tauri::Manager as _;
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(std::fs::read_to_string(dir.join("diag.log")).unwrap_or_default())
}

/// 清空诊断日志
#[tauri::command]
pub(crate) fn diag_clear(app: AppHandle) -> Result<(), String> {
    use tauri::Manager as _;
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(dir.join("diag.log"));
    Ok(())
}
