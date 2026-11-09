//! 跨源共享工具(重构提取):限速闸、静态正则、percent 解码、UA 常量。

use std::time::{Duration, Instant};

// ── 限速闸(niacg/hlib 双源同构提取) ──

/// 串行限速闸:内部记上次放行时刻,`throttle` 保证相邻放行间隔不小于
/// 设定值。锁跨 await 持有(串行化语义),多任务共享同一闸即全局限速。
pub struct RateGate {
    last: tokio::sync::Mutex<Instant>,
    min_interval: Duration,
}

impl RateGate {
    /// 创建即视为「刚放行过」(减去一个间隔),首个请求无需等待前置冷却
    pub fn new(min_interval: Duration) -> Self {
        Self {
            last: tokio::sync::Mutex::new(Instant::now() - min_interval),
            min_interval,
        }
    }

    /// 保证距上次放行至少 min_interval;不足则异步等待补齐
    pub async fn throttle(&self) {
        let mut last = self.last.lock().await;
        let elapsed = last.elapsed();
        if elapsed < self.min_interval {
            tokio::time::sleep(self.min_interval - elapsed).await;
        }
        *last = Instant::now();
    }
}

// ── 静态正则说明:OnceLock + get_or_init 是 Rust 惯用做法(评审后
// 保留现状,不做表式收敛) ──

// ── percent 解码(video.rs 手写版迁移,行为等价) ──

/// 简易 percent 解码(%XX 十六进制对;非法序列原样保留)
pub fn percent_decode_simple(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

// ── UA/指纹常量集中 ──

/// fs1 直连 UA(Jable 系,通用移动 Chrome)
pub const FS1_UA: &str =
    "Mozilla/5.0 (Linux; Android 13) AppleWebKit/537.36 Chrome/120 Mobile Safari/537.36";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_decode_basic_and_invalid() {
        assert_eq!(percent_decode_simple("%E7%80%AC%E6%88%B8"), "瀬戸");
        assert_eq!(percent_decode_simple("plain"), "plain");
        assert_eq!(percent_decode_simple("%zz"), "%zz");
        assert_eq!(percent_decode_simple("%4"), "%4");
    }

    #[tokio::test]
    async fn rate_gate_throttles() {
        let gate = RateGate::new(Duration::from_millis(50));
        let start = Instant::now();
        gate.throttle().await;
        gate.throttle().await;
        assert!(start.elapsed() >= Duration::from_millis(50));
    }
}
