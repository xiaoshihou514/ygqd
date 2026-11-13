import { getCurrentScope, onScopeDispose } from 'vue'

/**
 * 无限滚动哨兵(重构笔 2:七处 IntersectionObserver 手写模式收敛)。
 *
 * 用法:数据加载完成、哨兵元素渲染后调 setup();离开页面自动 teardown。
 * - target:哨兵元素获取器(querySelector 或模板 ref.value 均可,返回 null 视为未就绪)
 * - onHit:进入触发区回调(加载下一页)
 * - enabled:触发门槛(如 hasMore && !loading),setup 时判定,门槛不满足不绑定
 * - rootMargin:提前触发距离,默认 200px
 */
export interface InfiniteScrollOptions {
  target: () => Element | null
  onHit: () => void | Promise<void>
  enabled?: () => boolean
  rootMargin?: string
}

export function useInfiniteScroll(options: InfiniteScrollOptions) {
  let observer: IntersectionObserver | null = null

  /** 触发回调;同步异常与 Promise reject 统一记日志,不留未处理拒绝。
   *  (不在此加在途锁:调用方会在 onHit 内部重绑哨兵,锁会吞掉重绑后的首次回调) */
  function hit() {
    try {
      const result = options.onHit()
      if (result instanceof Promise) {
        result.catch((e: unknown) => console.error('[infinite-scroll] onHit 异常', e))
      }
    } catch (e) {
      console.error('[infinite-scroll] onHit 异常', e)
    }
  }

  /** 绑定哨兵(幂等:先拆旧再绑新;哨兵缺失或门槛不满足时不绑定) */
  function setup() {
    teardown()
    const el = options.target()
    if (!el) return
    if (options.enabled && !options.enabled()) return
    observer = new IntersectionObserver(
      (entries) => {
        // 同一哨兵一批可能含多次进出变化,以最后一条(当前状态)为准
        if (entries[entries.length - 1]?.isIntersecting) hit()
      },
      { rootMargin: options.rootMargin ?? '200px' },
    )
    observer.observe(el)
  }

  function teardown() {
    observer?.disconnect()
    observer = null
  }

  if (getCurrentScope()) onScopeDispose(teardown)
  return { setup, teardown }
}
