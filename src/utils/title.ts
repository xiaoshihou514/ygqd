/** 与 Rust `sources::shared::author_from_title` 同一口径(后端已归一,此处仅兜底旧数据):
 *  可跳过前置 `(C99)` 类括号组;取开头 `[作者 (别名)]` / `【作者】` 中括号前的部分。 */
const AUTHOR_RE = /^\s*(?:[(（][^)）]*[)）]\s*)*(?:\[([^\]]*)\]|【([^】]*)】)/

export function extractAuthorFromTitle(title: string): string | null {
  const m = title.match(AUTHOR_RE)
  const inner = m?.[1] ?? m?.[2]
  const name = inner?.split(/[(（]/)[0]?.trim()
  return name || null
}
