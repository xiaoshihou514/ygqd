export type ExportResult = 'shared' | 'downloaded' | 'copied' | 'cancelled'

interface JsonExportOptions {
  filename: string
  title: string
  data: unknown
}

function downloadFile(file: File): boolean {
  try {
    const url = URL.createObjectURL(file)
    const link = document.createElement('a')
    link.href = url
    link.download = file.name
    link.style.display = 'none'
    document.body.appendChild(link)
    link.click()
    link.remove()
    window.setTimeout(() => URL.revokeObjectURL(url), 1000)
    return true
  } catch {
    return false
  }
}

async function copyText(text: string): Promise<void> {
  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text)
    return
  }

  const textarea = document.createElement('textarea')
  textarea.value = text
  textarea.style.position = 'fixed'
  textarea.style.opacity = '0'
  document.body.appendChild(textarea)
  textarea.select()
  const copied = document.execCommand('copy')
  textarea.remove()
  if (!copied) throw new Error('Clipboard is unavailable')
}

export async function exportJson({ filename, title, data }: JsonExportOptions): Promise<ExportResult> {
  const json = `${JSON.stringify(data, null, 2)}\n`
  const file = new File([json], filename, { type: 'application/json' })

  if (navigator.share) {
    const shareData = { title, files: [file] }
    if (!navigator.canShare || navigator.canShare(shareData)) {
      try {
        await navigator.share(shareData)
        return 'shared'
      } catch (error) {
        if (error instanceof DOMException && error.name === 'AbortError') return 'cancelled'
      }
    }
  }

  if (downloadFile(file)) return 'downloaded'
  await copyText(json)
  return 'copied'
}

export function datedJsonFilename(stem: string): string {
  return `${stem}-${new Date().toISOString().slice(0, 10)}.json`
}
