import { onUnmounted } from 'vue'

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

  /** 绑定哨兵(幂等:先拆旧再绑新;哨兵缺失或门槛不满足时不绑定) */
  function setup() {
    teardown()
    const el = options.target()
    if (!el) return
    if (options.enabled && !options.enabled()) return
    observer = new IntersectionObserver(
      (entries) => {
        if (entries[0]?.isIntersecting) void options.onHit()
      },
      { rootMargin: options.rootMargin ?? '200px' },
    )
    observer.observe(el)
  }

  function teardown() {
    observer?.disconnect()
    observer = null
  }

  onUnmounted(teardown)
  return { setup, teardown }
}
