/** 计数展示格式化:中文习惯(>=1万 → X.X万 一位小数,千以内原样;无 12K 形态) */
export function formatCount(n: number): string {
  if (!Number.isFinite(n)) return '0'
  const sign = n < 0 ? '-' : ''
  const abs = Math.abs(n)
  if (abs >= 100000000) return `${sign}${trimTrailingZero(round1(abs / 100000000))}亿`
  if (abs >= 10000) {
    // 一位小数取整后可能溢出到上一档:99999999 → 10000.0万 应进位为「1亿」
    const wan = round1(abs / 10000)
    return wan >= 10000 ? `${sign}1亿` : `${sign}${trimTrailingZero(wan)}万`
  }
  const int = Math.round(abs)
  return int === 0 ? '0' : `${sign}${int}`
}

function round1(v: number): number {
  return Math.round(v * 10) / 10
}

function trimTrailingZero(v: number): string {
  const s = v.toFixed(1)
  return s.endsWith('.0') ? s.slice(0, -2) : s
}
