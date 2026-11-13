/** 常见底层错误(reqwest/wreq/serde 原文多为英文)→ 面向用户的中文文案。
 *  已含中文的错误(Rust 侧主动构造的提示)原样透传,不做改写。 */
const FRIENDLY_RULES: ReadonlyArray<readonly [RegExp, string]> = [
  [/timed? ?out|timeout|deadline has elapsed/i, '网络请求超时,请检查网络后重试'],
  [/dns error|failed to lookup address|name or service not known|no address associated|nodename nor servname/i, '无法解析服务器地址,请检查网络连接'],
  [/connection refused|connection reset|connection closed|connection aborted|broken pipe|network is unreachable|host is unreachable|error sending request|tcp connect error|tls|handshake|certificate/i, '网络连接失败,请检查网络或稍后重试'],
  [/\b(?:HTTP|status)[ :]*429\b|too many requests/i, '请求过于频繁,请稍后再试'],
  [/\b(?:HTTP|status)[ :]*403\b|forbidden/i, '源站拒绝访问(403),请稍后重试'],
  [/\b(?:HTTP|status)[ :]*404\b/i, '内容不存在或已下架(404)'],
  [/\b(?:HTTP|status)[ :]*5\d\d\b/i, '源站服务异常,请稍后重试'],
  [/error decoding response body|decode error|expected value at line|EOF while parsing|invalid type: |missing field/i, '数据解析失败,源站可能已改版'],
]

const CJK_RE = /[一-鿿]/

function rawMessage(e: unknown): string {
  if (e == null) return ''
  if (typeof e === 'string') return e
  if (e instanceof Error) return e.message
  if (typeof e === 'object' && 'message' in e && typeof (e as { message: unknown }).message === 'string') {
    return (e as { message: string }).message
  }
  return String(e)
}

/** 统一提取 invoke/异常的错误文案:Tauri invoke reject 抛出的是字符串
 *  (Rust 的 Err(String),无 .message 属性),其他来源可能是 Error 对象。
 *  常见网络/超时/解析类英文底层错误归一为中文提示;无法提取时返回空串
 *  (调用方 `errorMessage(e) || '加载失败'` 的兜底文案得以生效)。 */
export function errorMessage(e: unknown): string {
  const msg = rawMessage(e).trim()
  // Rust 侧以 "401" 前缀标记登录失效(可能带中文说明),须先于中文透传处理
  if (/^401\b/.test(msg)) return msg.replace(/^401\b[\s:：]*/, '') || '登录已失效,请重新登录'
  if (!msg || CJK_RE.test(msg)) return msg
  for (const [re, friendly] of FRIENDLY_RULES) {
    if (re.test(msg)) return friendly
  }
  return msg
}
