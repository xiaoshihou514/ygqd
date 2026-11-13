//! 诊断日志读取/清空 command(小说板块删除后,gateway 域仅存诊断)。
//! 文件读写在 tokio 线程池执行,不占主线程。

use crate::diagnostics;

/// 读取诊断日志(应用内查看)
#[tauri::command(async)]
pub(crate) fn diag_read() -> Result<String, String> {
    Ok(diagnostics::read_all())
}

/// 清空诊断日志
#[tauri::command(async)]
pub(crate) fn diag_clear() -> Result<(), String> {
    diagnostics::clear();
    Ok(())
}
