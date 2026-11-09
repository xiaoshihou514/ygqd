/**
 * 时间展示格式化(两个语义独立,不合并):
 * - formatRelativeTime:相对时间(「X 分钟前」)——关注页
 * - formatTimestamp:绝对时间(「MM-DD HH:mm」)——历史页
 */

/** 相对时间:分钟/小时/天前 */
export function formatRelativeTime(ms: number): string {
  const diff = Date.now() - ms
  const mins = Math.floor(diff / 60000)
  if (mins < 60) return `${mins} 分钟前`
  const hours = Math.floor(mins / 60)
  if (hours < 24) return `${hours} 小时前`
  const days = Math.floor(hours / 24)
  return `${days} 天前`
}

/** 绝对时间:MM-DD HH:mm */
export function formatTimestamp(ts: number): string {
  const d = new Date(ts)
  const M = String(d.getMonth() + 1).padStart(2, '0')
  const D = String(d.getDate()).padStart(2, '0')
  const hh = String(d.getHours()).padStart(2, '0')
  const mm = String(d.getMinutes()).padStart(2, '0')
  return `${M}-${D} ${hh}:${mm}`
}
