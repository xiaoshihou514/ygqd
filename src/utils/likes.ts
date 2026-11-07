/**
 * 解析站点计数文本为数字:兼容「1288」「1,288」「1 288」「1.2K」「3.4M」「1.2万」「3亿」
 * (千分位逗号/空格会让 parseFloat 截断,先剥掉)
 */
export function parseLikes(likes: string): number {
  if (!likes) return 0
  const cleaned = likes.trim().replace(/[,\s\u00a0\u202f]/g, '')
  if (!cleaned) return 0

  const num = parseFloat(cleaned)
  if (isNaN(num)) return 0
  if (/亿/.test(cleaned)) return num * 100000000
  if (/万/.test(cleaned)) return num * 10000
  if (/m/i.test(cleaned)) return num * 1000000
  if (/k/i.test(cleaned)) return num * 1000
  return num
}
