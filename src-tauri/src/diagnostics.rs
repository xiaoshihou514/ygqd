//! 应用诊断基础设施(diag.log 写入/读取/清空)。
//! 从 gateway.rs 迁出(小说板块删除,诊断为全 app 共用基础设施)。
//! 只用 setup 缓存的目录,任何线程(含 Android 主线程回调)调用都安全;
//! 未登记时静默丢弃。
//!
//! 体积有界:diag.log 超过 `MAX_LOG_BYTES` 时整体轮转为 diag.log.1
//! (覆盖旧的),读取时按时间顺序拼接两者,总量不超过约 2 倍上限。

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, PoisonError};

use tauri::AppHandle;

static DIAG_DIR: OnceLock<PathBuf> = OnceLock::new();
/// 串行化写入与轮转,避免并发轮转丢行
static WRITE_LOCK: Mutex<()> = Mutex::new(());

const LOG_FILE: &str = "diag.log";
const ROTATED_FILE: &str = "diag.log.1";
/// 单文件上限 512KB
const MAX_LOG_BYTES: u64 = 512 * 1024;

/// setup 阶段登记 diag 目录(先于任何网关导航)
pub fn init_diag_dir(dir: PathBuf) {
    let _ = DIAG_DIR.set(dir);
}

/// 诊断日志:写应用内部数据目录 diag.log;查看走应用内入口
/// (release 无法 run-as/logcat,应用内读取是唯一通道)。
pub fn diag(_app: &AppHandle, msg: &str) {
    if let Some(dir) = DIAG_DIR.get() {
        append(dir, msg);
    }
}

fn append(dir: &Path, msg: &str) {
    let _guard = WRITE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let path = dir.join(LOG_FILE);
    if std::fs::metadata(&path).is_ok_and(|m| m.len() >= MAX_LOG_BYTES) {
        let _ = std::fs::rename(&path, dir.join(ROTATED_FILE));
    }
    let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    else {
        return;
    };
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let _ = writeln!(f, "{ts} {msg}");
}

/// 读取全部诊断日志(轮转出的旧文件在前);目录未登记或无日志时为空串
pub fn read_all() -> String {
    let Some(dir) = DIAG_DIR.get() else {
        return String::new();
    };
    let _guard = WRITE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let mut out = std::fs::read_to_string(dir.join(ROTATED_FILE)).unwrap_or_default();
    out.push_str(&std::fs::read_to_string(dir.join(LOG_FILE)).unwrap_or_default());
    out
}

/// 清空诊断日志(含轮转文件)
pub fn clear() {
    let Some(dir) = DIAG_DIR.get() else {
        return;
    };
    let _guard = WRITE_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    let _ = std::fs::remove_file(dir.join(LOG_FILE));
    let _ = std::fs::remove_file(dir.join(ROTATED_FILE));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_rotates_when_over_limit() {
        let dir = std::env::temp_dir().join(format!("ygqd-diag-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let big = "x".repeat(MAX_LOG_BYTES as usize);
        std::fs::write(dir.join(LOG_FILE), &big).unwrap();
        append(&dir, "after rotate");
        let rotated = std::fs::metadata(dir.join(ROTATED_FILE)).unwrap().len();
        let current = std::fs::read_to_string(dir.join(LOG_FILE)).unwrap();
        assert_eq!(rotated, MAX_LOG_BYTES);
        assert!(current.ends_with("after rotate\n"));
        assert!((current.len() as u64) < MAX_LOG_BYTES);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
