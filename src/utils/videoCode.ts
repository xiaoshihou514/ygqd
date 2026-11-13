/**
 * 视频番号处理工具(需求 12 口径单点维护)。
 * 后缀语义:-chinese-subtitle(中字)/-uncensored-leak(无码流出)/
 * -uncensored-leak-chinese-subtitle(无码中字)。
 */

const SUFFIX_CS = '-chinese-subtitle'
const SUFFIX_LEAK = '-uncensored-leak'
/** 一次剥净全部变体后缀(组合后缀 = 两段依次出现) */
const SUFFIX_RE = /-uncensored-leak|-chinese-subtitle/g

/** 剥全部后缀并大写:展示用基础番号(MediaCard 适配器与各页共用)。
 *  先统一小写再匹配,大写来源的番号同样能剥净后缀 */
export function videoBaseCode(code: string): string {
  return code.toLowerCase().replace(SUFFIX_RE, '').toUpperCase()
}

/** base_code 去重优先级(需求 12 归并口径):中字 > 有码 > 无码 */
function dedupePriority(code: string): number {
  const c = code.toLowerCase()
  if (c.includes(SUFFIX_CS)) return 3
  if (!c.includes(SUFFIX_LEAK)) return 2
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
