import { invoke } from '@tauri-apps/api/core'

/**
 * hls.js 自定义 loader:经 Tauri invoke 走 Rust 转发
 * (CDN 校验 Referer/TLS,浏览器直连必 403;自定义 scheme fetch
 * 在 webkit 上不可靠,invoke 是原生通道必然可用)。
 *
 * hls.js 的 loader 契约未导出为公共类型,这里以最小结构对接
 * v1 loader(load/abort/destroy + stats),回调签名与内置
 * fetch/xhr loader 一致: onSuccess(response, stats, context) /
 * onError(error, context, networkDetails, stats)
 */
/* eslint-disable @typescript-eslint/no-explicit-any */
function makeStats(total: number): any {
  const now = performance.now()
  return {
    loading: { start: now, first: now, end: now },
    total,
    loaded: total,
    aborted: false,
    retry: 0,
    chunkCount: 1,
    bwEstimate: 0,
  }
}

export function configureHls(hls: any) {
  class TauriLoader {
    private ctx: any = null
    private aborted = false

    constructor(_config: any) {
      void _config
    }

    load(context: any, _config: any, callbacks: any) {
      this.ctx = context
      this.aborted = false
      void (async () => {
        try {
          const payload = await invoke<{
            playlist: string | null
            bytes: number[]
            contentType: string
          }>('hls_fetch', { url: context.url })
          if (this.aborted) return
          if (payload.playlist !== null) {
            const data = payload.playlist
            callbacks.onSuccess(
              { url: context.url, data },
              makeStats(data.length),
              context,
              null,
            )
          } else {
            const bytes = new Uint8Array(payload.bytes)
            callbacks.onSuccess(
              { url: context.url, data: bytes.buffer },
              makeStats(bytes.length),
              context,
              null,
            )
          }
        } catch (e) {
          if (this.aborted) return
          callbacks.onError(
            {
              code: 0,
              details: 'fragLoadError',
              fatal: false,
              response: { url: context.url, data: undefined },
              networkDetails: String(e),
            },
            context,
            null,
            makeStats(0),
          )
        }
      })()
    }

    abort() {
      this.aborted = true
    }

    destroy() {
      this.aborted = true
    }

    get stats() {
      return makeStats(0)
    }

    get context() {
      return this.ctx
    }

    set context(value: any) {
      this.ctx = value
    }
  }

  hls.config.loader = TauriLoader
}
