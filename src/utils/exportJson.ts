export type ExportResult = 'shared' | 'downloaded' | 'copied' | 'cancelled'

interface JsonExportOptions {
  filename: string
  title: string
  data: unknown
}

/** Blob URL 保留时长:部分 WebView 在 click 后异步开始下载,过早 revoke 会拿到空文件 */
const OBJECT_URL_TTL_MS = 30_000

function downloadFile(file: File): boolean {
  let url: string | null = null
  try {
    url = URL.createObjectURL(file)
    const link = document.createElement('a')
    link.href = url
    link.download = file.name
    link.rel = 'noopener'
    link.style.display = 'none'
    document.body.appendChild(link)
    try {
      link.click()
    } finally {
      link.remove()
    }
    const toRevoke = url
    window.setTimeout(() => URL.revokeObjectURL(toRevoke), OBJECT_URL_TTL_MS)
    return true
  } catch {
    if (url) URL.revokeObjectURL(url)
    return false
  }
}

/** 旧 WebView 没有异步剪贴板 API 时退回 execCommand;任何一步失败都抛错交给调用方 */
async function copyText(text: string): Promise<void> {
  if (navigator.clipboard?.writeText) {
    try {
      await navigator.clipboard.writeText(text)
      return
    } catch {
      // 权限被拒(非安全上下文/无用户激活)时继续尝试 execCommand
    }
  }

  const active = document.activeElement as HTMLElement | null
  const textarea = document.createElement('textarea')
  textarea.value = text
  textarea.readOnly = true
  textarea.style.position = 'fixed'
  textarea.style.top = '0'
  textarea.style.left = '0'
  textarea.style.opacity = '0'
  document.body.appendChild(textarea)
  let copied = false
  try {
    textarea.select()
    copied = document.execCommand('copy')
  } catch {
    copied = false
  } finally {
    textarea.remove()
    active?.focus?.()
  }
  if (!copied) throw new Error('Clipboard is unavailable')
}

function canShareFiles(shareData: ShareData): boolean {
  if (typeof navigator.share !== 'function') return false
  if (typeof navigator.canShare !== 'function') return true
  try {
    return navigator.canShare(shareData)
  } catch {
    return false
  }
}

/**
 * 导出 JSON:优先系统分享(Android),其次浏览器下载,最后复制到剪贴板。
 * 用户主动取消分享返回 'cancelled';全部失败时抛错,由调用方提示。
 */
export async function exportJson({ filename, title, data }: JsonExportOptions): Promise<ExportResult> {
  const json = `${JSON.stringify(data, null, 2)}\n`
  const file = new File([json], filename, { type: 'application/json' })

  const shareData: ShareData = { title, files: [file] }
  if (canShareFiles(shareData)) {
    try {
      await navigator.share(shareData)
      return 'shared'
    } catch (error) {
      if (error instanceof DOMException && error.name === 'AbortError') return 'cancelled'
      // 其他错误(如用户激活已过期 NotAllowedError)继续走下载兜底
    }
  }

  if (downloadFile(file)) return 'downloaded'
  await copyText(json)
  return 'copied'
}

const pad2 = (n: number) => String(n).padStart(2, '0')

/** `${stem}-YYYY-MM-DD-HHmmss.json`,按本地时区(toISOString 是 UTC,东八区凌晨会差一天);
 *  带时分秒避免同日多次导出互相覆盖或被系统追加 (1) 后缀。 */
export function datedJsonFilename(stem: string): string {
  const d = new Date()
  const date = `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}`
  const time = `${pad2(d.getHours())}${pad2(d.getMinutes())}${pad2(d.getSeconds())}`
  return `${stem}-${date}-${time}.json`
}
