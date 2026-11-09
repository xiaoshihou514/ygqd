/**
 * hlib 限速守卫(重构笔 3:两页同语义不同实现收敛)。
 *
 * 站点反爬闸:相邻抓取保底 3 秒间隔(不足由等待补齐)。
 * **每调用返回独立计时实例**——NovelPage 与 SearchPage 各自独立节流,
 * 行为与重构前一致(交替使用互相不限速)。
 */
const HLIB_MIN_INTERVAL_MS = 3000

export interface HlibGuard {
  /** 距上次抓取不足 3 秒时等待补齐,并更新计时 */
  guard: () => Promise<void>
  /** 上次抓取时间戳(冷却倒计时展示用) */
  lastAt: () => number
  /** 间隔常量(毫秒) */
  intervalMs: number
}

export function useHlibGuard(): HlibGuard {
  let lastFetchAt = 0

  async function guard(): Promise<void> {
    const elapsed = Date.now() - lastFetchAt
    if (elapsed < HLIB_MIN_INTERVAL_MS) {
      await new Promise((resolve) => setTimeout(resolve, HLIB_MIN_INTERVAL_MS - elapsed))
    }
    lastFetchAt = Date.now()
  }

  return {
    guard,
    lastAt: () => lastFetchAt,
    intervalMs: HLIB_MIN_INTERVAL_MS,
  }
}
