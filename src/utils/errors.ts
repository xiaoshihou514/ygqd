/** 统一提取 invoke/异常的错误文案:Tauri invoke reject 抛出的是字符串
 *  (Rust 的 Err(String),无 .message 属性),其他来源可能是 Error 对象。 */
export function errorMessage(e: unknown): string {
  if (typeof e === 'string') return e
  if (e instanceof Error) return e.message
  return String(e)
}
