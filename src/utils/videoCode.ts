/**
 * 视频番号处理工具(需求 12 口径单点维护)。
 * 后缀语义:-chinese-subtitle(中字)/-uncensored-leak(无码流出)/
 * -uncensored-leak-chinese-subtitle(无码中字)。
 */

/** 剥全部后缀并大写:展示用基础番号(VideoBrowser/VideoCardGrid 共用) */
export function videoBaseCode(code: string): string {
  return code
    .replace('-uncensored-leak-chinese-subtitle', '')
    .replace('-uncensored-leak', '')
    .replace('-chinese-subtitle', '')
    .toUpperCase()
}

/** base_code 去重优先级(需求 12 归并口径):中字 > 有码 > 无码 */
function dedupePriority(code: string): number {
  if (code.includes('-chinese-subtitle')) return 3
  if (!code.includes('-uncensored-leak')) return 2
  return 1
}

/** 按 base_code 归并去重(同番号多变体保留优先级最高的一条) */
export function dedupeByVideoBase<T extends { code: string }>(items: readonly T[]): T[] {
  const best = new Map<string, T>()
  for (const item of items) {
    const base = videoBaseCode(item.code)
    const current = best.get(base)
    if (!current || dedupePriority(item.code) > dedupePriority(current.code)) {
      best.set(base, item)
    }
  }
  return [...best.values()]
}
