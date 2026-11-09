//! gateway 域 command(重构笔 3:按源分组)

use crate::gateway::{self, GatewayManager};
use tauri::AppHandle;
use tauri::Manager;
use tauri::State;

/// 通过 WebView 网关抓取页面/接口(用于 CF 严防站点: hlib/missav)。
/// 首次调用会创建可见网关窗口,若遇人机挑战需用户点一次。
#[tauri::command]
pub(crate) async fn gateway_fetch(
    app: AppHandle,
    state: State<'_, GatewayManager>,
    label: String,
    url: String,
) -> Result<String, String> {
    gateway::gateway_fetch(&app, &state, &label, &url).await
}

/// 打开网关窗口(不存在则创建):供用户登录站点/完成人机验证,
/// 或在网关内直接浏览站点内容
#[tauri::command]
pub(crate) fn gateway_open(app: AppHandle, label: String) -> Result<(), String> {
    gateway::ensure_gateway(&app, &label)?;
    let window = app
        .get_webview_window(&format!("gateway_{label}"))
        .ok_or("网关窗口不存在")?;
    gateway::present(&window);
    Ok(())
}

/// 显示/隐藏网关窗口(挑战已过后隐藏)
#[tauri::command]
pub(crate) fn gateway_set_visible(
    app: AppHandle,
    label: String,
    visible: bool,
) -> Result<(), String> {
    let window = app
        .get_webview_window(&format!("gateway_{label}"))
        .ok_or("网关窗口不存在")?;
    if visible {
        gateway::present(&window);
        Ok(())
    } else {
        gateway::finish(&app, &label)
    }
}

/// 完成网关操作(用户点了网关内「完成返回」栏):隐藏网关回主界面
#[tauri::command]
pub(crate) fn gateway_finish(app: AppHandle, label: String) -> Result<(), String> {
    gateway::diag(&app, "[通道观测] gwfinish 经 IPC 通道到达");
    gateway::finish(&app, &label)
}

/// 网关桥回传(由网关 webview 内的 JS 调用,不暴露给主窗口前端)
#[tauri::command]
pub(crate) fn gateway_result(
    app: AppHandle,
    state: State<'_, GatewayManager>,
    result: gateway::GatewayReply,
) {
    gateway::diag(
        &app,
        &format!(
            "[通道观测] gwresult 经 IPC 通道到达 id={} status={} body={}B",
            result.id,
            result.status,
            result.body.len()
        ),
    );
    state.resolve_reply(result);
}

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
