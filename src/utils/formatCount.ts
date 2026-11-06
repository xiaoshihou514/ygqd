/** 计数展示格式化:中文习惯(>=1万 → X.X万 一位小数,千以内原样;无 12K 形态) */
export function formatCount(n: number): string {
  if (!Number.isFinite(n)) return '0'
  if (n >= 100000000) {
    return `${trimTrailingZero(n / 100000000)}亿`
  }
  if (n >= 10000) {
    return `${trimTrailingZero(n / 10000)}万`
  }
  return String(Math.round(n))
}

function trimTrailingZero(v: number): string {
  const s = v.toFixed(1)
  return s.endsWith('.0') ? s.slice(0, -2) : s
}
