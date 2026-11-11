//! 应用诊断基础设施(diag.log 写入/读取/清空)。
//! 从 gateway.rs 迁出(小说板块删除,诊断为全 app 共用基础设施)。
//! 只用 setup 缓存的目录,任何线程(含 Android 主线程回调)调用都安全;
//! 未登记时静默丢弃。

use std::io::Write as _;

use tauri::AppHandle;

static DIAG_DIR: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

/// setup 阶段登记 diag 目录(先于任何网关导航)
pub fn init_diag_dir(dir: std::path::PathBuf) {
    let _ = DIAG_DIR.set(dir);
}

/// 诊断日志:写应用内部数据目录 diag.log;查看走应用内入口
/// (release 无法 run-as/logcat,应用内读取是唯一通道)。
/// 只用 setup 缓存的目录,任何线程(含 Android 主线程回调)调用都安全;
/// 未登记时静默丢弃。

pub fn diag(_app: &AppHandle, msg: &str) {
    let Some(dir) = DIAG_DIR.get() else {
        return;
    };
    let _ = std::fs::create_dir_all(dir);
    let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("diag.log"))
    else {
        return;
    };
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let _ = writeln!(f, "{ts} {msg}");
}

// tauri 宏重导出(generate_handler 跨模块引用需要)
