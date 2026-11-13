/**
 * 时间展示格式化(两个语义独立,不合并):
 * - formatRelativeTime:相对时间(「X 分钟前」)
 * - formatTimestamp:绝对时间(「MM-DD HH:mm」)——历史页/卡片行布局
 */

const MINUTE = 60_000
const HOUR = 60 * MINUTE
const DAY = 24 * HOUR

/** 相对时间:刚刚/分钟/小时/天前(未来时间戳——设备时钟回拨等——按「刚刚」处理) */
export function formatRelativeTime(ms: number): string {
  if (!Number.isFinite(ms)) return ''
  const diff = Math.max(0, Date.now() - ms)
  if (diff < MINUTE) return '刚刚'
  if (diff < HOUR) return `${Math.floor(diff / MINUTE)} 分钟前`
  if (diff < DAY) return `${Math.floor(diff / HOUR)} 小时前`
  return `${Math.floor(diff / DAY)} 天前`
}

const pad2 = (n: number) => String(n).padStart(2, '0')

/** 绝对时间:MM-DD HH:mm(非今年的记录前置年份避免跨年歧义;非法时间戳返回空串) */
export function formatTimestamp(ts: number): string {
  if (!Number.isFinite(ts)) return ''
  const d = new Date(ts)
  if (Number.isNaN(d.getTime())) return ''
  const md = `${pad2(d.getMonth() + 1)}-${pad2(d.getDate())} ${pad2(d.getHours())}:${pad2(d.getMinutes())}`
  return d.getFullYear() === new Date().getFullYear() ? md : `${d.getFullYear()}-${md}`
}
