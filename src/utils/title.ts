const BRACKET_RE = /^[\[【]([^\]】]+)[\]】]/

export function extractAuthorFromTitle(title: string): string | null {
  const match = title.match(BRACKET_RE)
  return match ? match[1]!.trim() : null
}
