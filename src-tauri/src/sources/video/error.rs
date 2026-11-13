//! 上游请求错误归类。
//!
//! 返回给前端的错误串会被页面原样展示,因此只保留「哪个源 + 什么问题」
//! 的简短中文(如 `fs1 请求超时`、`missav 拒绝访问(HTTP 403)`);
//! 请求地址与底层错误链等调试细节写 stderr(桌面调试见 app.log)。

/// 视频源名(出现在错误串开头,与 `VideoDetail.provider` 一致)
pub(super) const FS1: &str = "fs1";
pub(super) const MISSAV: &str = "missav";

/// 状态码 → 用户可读描述
pub(super) fn status_message(source: &str, code: u16) -> String {
    match code {
        403 => format!("{source} 拒绝访问(HTTP 403)"),
        404 => format!("{source} 未找到该内容(HTTP 404)"),
        429 => format!("{source} 请求过于频繁(HTTP 429)"),
        500..=599 => format!("{source} 服务器异常(HTTP {code})"),
        _ => format!("{source} 请求失败(HTTP {code})"),
    }
}

/// 两个 HTTP 客户端(reqwest / wreq)错误的共同判别面
pub(super) trait NetError: std::fmt::Debug {
    fn timed_out(&self) -> bool;
    fn connect_failed(&self) -> bool;
    fn status_code(&self) -> Option<u16>;
    fn body_failed(&self) -> bool;
}

impl NetError for reqwest::Error {
    fn timed_out(&self) -> bool {
        self.is_timeout()
    }
    fn connect_failed(&self) -> bool {
        self.is_connect()
    }
    fn status_code(&self) -> Option<u16> {
        self.status().map(|s| s.as_u16())
    }
    fn body_failed(&self) -> bool {
        self.is_body() || self.is_decode()
    }
}

impl NetError for wreq::Error {
    fn timed_out(&self) -> bool {
        self.is_timeout()
    }
    fn connect_failed(&self) -> bool {
        self.is_connect() || self.is_connection_reset()
    }
    fn status_code(&self) -> Option<u16> {
        self.status().map(|s| s.as_u16())
    }
    fn body_failed(&self) -> bool {
        self.is_body() || self.is_decode()
    }
}

/// 网络层错误 → 简短中文;完整错误(含 URL 与错误链)写日志
pub(super) fn net_error(source: &str, url: &str, e: &impl NetError) -> String {
    log(source, url, e);
    if e.timed_out() {
        format!("{source} 请求超时")
    } else if e.connect_failed() {
        format!("{source} 连接失败,请检查网络")
    } else if let Some(code) = e.status_code() {
        status_message(source, code)
    } else if e.body_failed() {
        format!("{source} 响应读取失败")
    } else {
        format!("{source} 请求失败")
    }
}

/// 调试细节日志(不进入返回给前端的错误串)
pub(super) fn log(source: &str, url: &str, detail: &dyn std::fmt::Debug) {
    eprintln!("[video:{source}] {url}: {detail:?}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_messages_are_concise() {
        assert_eq!(status_message(FS1, 404), "fs1 未找到该内容(HTTP 404)");
        assert_eq!(status_message(MISSAV, 403), "missav 拒绝访问(HTTP 403)");
        assert_eq!(status_message(FS1, 502), "fs1 服务器异常(HTTP 502)");
        assert_eq!(status_message(FS1, 418), "fs1 请求失败(HTTP 418)");
    }
}
