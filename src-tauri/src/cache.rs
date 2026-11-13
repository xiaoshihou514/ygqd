//! 进程内 TTL 缓存(列表/搜索/详情等抓取结果)。
//!
//! - 值以 `Arc<dyn Any>` 原样保存,命中时 downcast 后 clone,免去
//!   serde_json 往返序列化;
//! - 容量有界:超限时先清过期项,仍超限则淘汰最早写入的条目;
//! - 相同 key 的并发未命中合并为一次上游请求(singleflight),
//!   结果(含错误)共享给同批等待者,错误不进入缓存;
//! - 所有同步锁只在临界区内持有,不跨 await。

use std::{
    any::Any,
    collections::HashMap,
    future::Future,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, PoisonError, RwLock,
    },
    time::{Duration, Instant},
};
use tokio::sync::OnceCell;

/// 缓存有效期
pub const CACHE_TTL: Duration = Duration::from_secs(300);
/// 条目上限(单条为一页列表/详情,量级 10~100KB)
pub const CACHE_CAPACITY: usize = 256;

type Value = Arc<dyn Any + Send + Sync>;
type Flight = Arc<OnceCell<Result<Value, String>>>;

struct Entry {
    at: Instant,
    value: Value,
}

pub struct TtlCache {
    ttl: Duration,
    capacity: usize,
    entries: RwLock<HashMap<String, Entry>>,
    inflight: std::sync::Mutex<HashMap<String, Flight>>,
    /// clear 时自增;清空前发起的请求完成后不再回写,避免旧登录态/旧分流的结果复活
    generation: AtomicU64,
}

impl TtlCache {
    pub fn new(ttl: Duration, capacity: usize) -> Self {
        Self {
            ttl,
            capacity: capacity.max(1),
            entries: RwLock::new(HashMap::new()),
            inflight: std::sync::Mutex::new(HashMap::new()),
            generation: AtomicU64::new(0),
        }
    }

    fn get_value(&self, key: &str) -> Option<Value> {
        let entries = self.entries.read().unwrap_or_else(PoisonError::into_inner);
        let entry = entries.get(key)?;
        (entry.at.elapsed() < self.ttl).then(|| entry.value.clone())
    }

    /// 读取未过期的缓存值
    pub fn get<T: Clone + 'static>(&self, key: &str) -> Option<T> {
        self.get_value(key)?.downcast_ref::<T>().cloned()
    }

    fn put_value(&self, key: String, value: Value) {
        let mut entries = self.entries.write().unwrap_or_else(PoisonError::into_inner);
        entries.insert(
            key,
            Entry {
                at: Instant::now(),
                value,
            },
        );
        if entries.len() > self.capacity {
            let ttl = self.ttl;
            entries.retain(|_, e| e.at.elapsed() < ttl);
        }
        while entries.len() > self.capacity {
            let Some(oldest) = entries
                .iter()
                .min_by_key(|(_, e)| e.at)
                .map(|(k, _)| k.clone())
            else {
                break;
            };
            entries.remove(&oldest);
        }
    }

    /// 写入缓存
    #[cfg(test)]
    pub fn put<T: Send + Sync + 'static>(&self, key: String, value: T) {
        self.put_value(key, Arc::new(value));
    }

    /// 清空缓存(登录态/分流变化时调用),并让进行中的请求结果失效
    pub fn clear(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.entries
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
        self.inflight
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
    }

    /// 命中直接返回;未命中执行 `fetch` 并缓存成功结果。
    /// 同一 key 的并发调用只执行一次 `fetch`,其余等待并共享结果。
    pub async fn get_or_fetch<T, Fut>(&self, key: String, fetch: Fut) -> Result<T, String>
    where
        T: Clone + Send + Sync + 'static,
        Fut: Future<Output = Result<T, String>>,
    {
        if let Some(v) = self.get::<T>(&key) {
            return Ok(v);
        }
        let generation = self.generation.load(Ordering::SeqCst);
        let flight: Flight = self
            .inflight
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entry(key.clone())
            .or_default()
            .clone();
        let result = flight
            .get_or_init(|| async {
                // 可能刚有一轮请求完成并写入缓存
                if let Some(v) = self.get_value(&key) {
                    return Ok(v);
                }
                let value: Value = Arc::new(fetch.await?);
                if self.generation.load(Ordering::SeqCst) == generation {
                    self.put_value(key.clone(), value.clone());
                }
                Ok(value)
            })
            .await
            .clone();
        {
            let mut inflight = self.inflight.lock().unwrap_or_else(PoisonError::into_inner);
            if inflight.get(&key).is_some_and(|f| Arc::ptr_eq(f, &flight)) {
                inflight.remove(&key);
            }
        }
        result?
            .downcast_ref::<T>()
            .cloned()
            .ok_or_else(|| format!("缓存类型不匹配: {key}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    fn block_on<F: Future>(f: F) -> F::Output {
        tauri::async_runtime::block_on(f)
    }

    #[test]
    fn hit_and_type_mismatch() {
        let c = TtlCache::new(CACHE_TTL, 8);
        c.put("a".into(), vec![1, 2, 3]);
        assert_eq!(c.get::<Vec<i32>>("a"), Some(vec![1, 2, 3]));
        assert_eq!(c.get::<String>("a"), None);
        assert_eq!(c.get::<Vec<i32>>("missing"), None);
    }

    #[test]
    fn expired_entries_are_ignored() {
        let c = TtlCache::new(Duration::ZERO, 8);
        c.put("a".into(), 1u8);
        assert_eq!(c.get::<u8>("a"), None);
    }

    #[test]
    fn capacity_is_bounded_and_evicts_oldest() {
        let c = TtlCache::new(CACHE_TTL, 3);
        for i in 0..10u32 {
            c.put(format!("k{i}"), i);
        }
        assert_eq!(c.entries.read().unwrap().len(), 3);
        assert_eq!(c.get::<u32>("k0"), None);
        assert_eq!(c.get::<u32>("k9"), Some(9));
    }

    #[test]
    fn fetch_caches_success_but_not_error() {
        let c = TtlCache::new(CACHE_TTL, 8);
        let calls = AtomicUsize::new(0);
        let fetch = || async {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok::<_, String>(7u8)
        };
        assert_eq!(block_on(c.get_or_fetch("k".into(), fetch())), Ok(7));
        assert_eq!(block_on(c.get_or_fetch("k".into(), fetch())), Ok(7));
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        let err = block_on(c.get_or_fetch::<u8, _>("e".into(), async { Err("x".into()) }));
        assert_eq!(err, Err("x".into()));
        assert_eq!(block_on(c.get_or_fetch("e".into(), fetch())), Ok(7));
        assert!(c.inflight.lock().unwrap().is_empty());
    }

    #[test]
    fn concurrent_identical_requests_share_one_fetch() {
        let c = TtlCache::new(CACHE_TTL, 8);
        let calls = AtomicUsize::new(0);
        let fetch = || async {
            calls.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(30)).await;
            Ok::<_, String>(String::from("v"))
        };
        let (a, b, d) = block_on(async {
            tokio::join!(
                c.get_or_fetch("k".into(), fetch()),
                c.get_or_fetch("k".into(), fetch()),
                c.get_or_fetch("k".into(), fetch()),
            )
        });
        assert_eq!((a, b, d), (Ok("v".into()), Ok("v".into()), Ok("v".into())));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn clear_drops_results_of_older_flights() {
        let c = TtlCache::new(CACHE_TTL, 8);
        let v = block_on(c.get_or_fetch("k".into(), async {
            c.clear();
            Ok::<_, String>(1u8)
        }));
        assert_eq!(v, Ok(1));
        assert_eq!(c.get::<u8>("k"), None);
    }
}
