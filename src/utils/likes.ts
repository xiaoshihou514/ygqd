export function parseLikes(likes: string): number {
  if (!likes) return 0
  const cleaned = likes.trim()
  if (!cleaned) return 0

  const num = parseFloat(cleaned)
  if (!isNaN(num)) {
    if (/万/i.test(cleaned)) return num * 10000
    if (/k/i.test(cleaned)) return num * 1000
    return num
  }

  return 0
}
