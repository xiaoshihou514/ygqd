import { fileURLToPath, URL } from 'node:url'
import { connect, type TLSSocket } from 'node:tls'
import { brotliDecompressSync } from 'node:zlib'
import { get as httpsGet } from 'node:https'
import { get as httpGet } from 'node:http'
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import vueDevTools from 'vite-plugin-vue-devtools'
import { parseHomepage, parseListPage, parseSearchResults, parseComicDetail, parseComicImages } from './src/server/parser'
import { parse } from 'node-html-parser'
import type { SearchResult } from './src/server/parser'
import type { ComicItem, HomeSection, ComicDetail } from './src/types'

const NIACG_HOST = 'www.niacg.com'
const NIACG_BASE = `https://${NIACG_HOST}`

const CHROME_CIPHERS = [
  'TLS_AES_128_GCM_SHA256',
  'TLS_AES_256_GCM_SHA384',
  'TLS_CHACHA20_POLY1305_SHA256',
  'ECDHE-ECDSA-AES128-GCM-SHA256',
  'ECDHE-RSA-AES128-GCM-SHA256',
  'ECDHE-ECDSA-AES256-GCM-SHA384',
  'ECDHE-RSA-AES256-GCM-SHA384',
].join(':')

const UA =
  'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36'

const TLS_OPTS = {
  host: NIACG_HOST,
  port: 443,
  servername: NIACG_HOST,
  ALPNProtocols: ['http/1.1'] as string[],
  ciphers: CHROME_CIPHERS,
  honorCipherOrder: true,
  minVersion: 'TLSv1.2' as const,
  maxVersion: 'TLSv1.3' as const,
  rejectUnauthorized: false,
}

interface TlsResponse {
  status: number
  headers: Record<string, string>
  body: string
  setCookies: string[]
}

const URL_PATTERN = /^https?:\/\//i

function proxyThumbnail(url: string): string {
  if (!url) return ''
  return `/api/image?url=${encodeURIComponent(url)}`
}

function rewriteThumbnailsInItems(items: ComicItem[]) {
  for (const item of items) {
    if (item.thumbnail) {
      item.thumbnail = proxyThumbnail(item.thumbnail)
    }
  }
}

function rewriteThumbnailsInSections(sections: HomeSection[]) {
  for (const section of sections) {
    rewriteThumbnailsInItems(section.items)
  }
}

function proxyImage(imageUrl: string): Promise<{ mime: string; data: Buffer }> {
  return new Promise((resolve, reject) => {
    const parsed = new URL(imageUrl)
    const getter = parsed.protocol === 'https:' ? httpsGet : httpGet
    const opts = {
      hostname: parsed.hostname,
      port: parsed.port,
      path: parsed.pathname + parsed.search,
      headers: {
        'User-Agent': UA,
        Referer: NIACG_BASE + '/',
        Accept: 'image/avif,image/webp,image/apng,image/*,*/*;q=0.8',
      },
    }
    const req = getter(opts, (imgRes) => {
      const chunks: Buffer[] = []
      imgRes.on('data', (c: Buffer) => chunks.push(c))
      imgRes.on('end', () => {
        const mime = imgRes.headers['content-type'] || 'image/jpeg'
        resolve({ mime, data: Buffer.concat(chunks) })
      })
    })
    req.on('error', reject)
    req.setTimeout(15000, () => {
      req.destroy()
      reject(new Error('Image proxy timeout'))
    })
  })
}

function extractTagSearchMaxPage(html: string, keyword: string): number {
  const root = parse(html)
  const pageLinks = root.querySelectorAll('.pagination a')
  const urls = pageLinks.map((a) => a.getAttribute('href') || '').filter(Boolean)

  const escapedKeyword = keyword.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const regex = new RegExp(`tags-${escapedKeyword}-(\\d+)\\.html`)

  const pageNums = urls
    .map((u) => {
      const m = u.match(regex)
      return m ? Number(m[1]) : NaN
    })
    .filter((n) => !isNaN(n))

  return pageNums.length > 0 ? Math.max(...pageNums) : 0
}

function tlsRequest(
  method: string,
  path: string,
  extraHeaders?: Record<string, string>,
  body?: string,
  maxRedirects = 5,
  cookies?: string[],
): Promise<TlsResponse> {
  return new Promise((resolve, reject) => {
    const sock: TLSSocket = connect(TLS_OPTS, () => {
      const lines = [
        `${method} ${path} HTTP/1.1`,
        `Host: ${NIACG_HOST}`,
        `User-Agent: ${UA}`,
        'Accept: text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8',
        'Accept-Language: zh-CN,zh;q=0.9,en;q=0.8',
        'Connection: close',
      ]

      if (cookies && cookies.length > 0) {
        lines.push(`Cookie: ${cookies.join('; ')}`)
      }

      if (body) {
        lines.push('Content-Type: application/x-www-form-urlencoded')
        lines.push(`Content-Length: ${Buffer.byteLength(body)}`)
      }

      if (extraHeaders) {
        for (const [k, v] of Object.entries(extraHeaders)) {
          lines.push(`${k}: ${v}`)
        }
      }

      sock.write(lines.join('\r\n') + '\r\n\r\n')
      if (body) sock.write(body)
    })

    const chunks: Buffer[] = []
    sock.on('data', (c: Buffer) => chunks.push(c))
    sock.on('end', () => {
      const raw = Buffer.concat(chunks)
      const headerEnd = raw.indexOf('\r\n\r\n')
      if (headerEnd === -1) {
        reject(new Error('No header end found'))
        return
      }

      const headerPart = raw.subarray(0, headerEnd).toString('utf8')
      let bodyBuf = raw.subarray(headerEnd + 4)

      const headerLines = headerPart.split('\r\n')
      const statusLine = headerLines[0]
      const status = parseInt(statusLine.split(' ')[1], 10)

      const headers: Record<string, string> = {}
      const setCookies: string[] = []
      for (const line of headerLines.slice(1)) {
        const colIdx = line.indexOf(': ')
        if (colIdx === -1) continue
        const key = line.substring(0, colIdx).toLowerCase()
        const value = line.substring(colIdx + 2)

        if (key === 'set-cookie') {
          setCookies.push(value.split(';')[0])
        } else {
          headers[key] = value
        }
      }

      const allCookies = [...(cookies || []), ...setCookies]

      const isRedirect = status === 301 || status === 302 || status === 303
      if (isRedirect && headers['location'] && maxRedirects > 0) {
        let redirectPath = headers['location']
        if (URL_PATTERN.test(redirectPath)) {
          const u = new URL(redirectPath)
          redirectPath = u.pathname + u.search
        } else if (!redirectPath.startsWith('/')) {
          const base = path.endsWith('/') ? path : path.substring(0, path.lastIndexOf('/') + 1)
          redirectPath = base + redirectPath
        }
        tlsRequest('GET', redirectPath, undefined, undefined, maxRedirects - 1, allCookies)
          .then(resolve)
          .catch(reject)
        return
      }

      const isChunked = headers['transfer-encoding'] === 'chunked'
      if (isChunked) {
        const decodedChunks: Buffer[] = []
        let i = 0
        while (i < bodyBuf.length) {
          const crlf = bodyBuf.indexOf('\r\n', i, 'utf8')
          if (crlf === -1) break
          const chunkSize = parseInt(bodyBuf.subarray(i, crlf).toString('utf8'), 16)
          if (chunkSize === 0 || isNaN(chunkSize)) break
          const dataStart = crlf + 2
          const dataEnd = Math.min(dataStart + chunkSize, bodyBuf.length)
          decodedChunks.push(bodyBuf.subarray(dataStart, dataEnd))
          i = dataEnd + 2
        }
        bodyBuf = Buffer.concat(decodedChunks)
      }

      const encoding = headers['content-encoding']
      if (encoding === 'br') {
        bodyBuf = brotliDecompressSync(bodyBuf)
      } else if (encoding === 'gzip') {
        bodyBuf = require('node:zlib').gunzipSync(bodyBuf)
      }

      const responseBody = bodyBuf.toString('utf8')
      resolve({ status, headers, body: responseBody, setCookies })
    })

    sock.on('error', reject)
  })
}

export default defineConfig({
  plugins: [
    vue(),
    vueDevTools(),
    {
      name: 'niacg-proxy',
      configureServer(server) {
        server.middlewares.use(async (req, res, next) => {
          const url = req.url || ''

          if (url.startsWith('/api/image')) {
            const urlObj = new URL(url, 'http://localhost')
            const imageUrl = urlObj.searchParams.get('url')
            if (!imageUrl) {
              res.statusCode = 400
              res.end('Missing url parameter')
              return
            }
            try {
              const { mime, data } = await proxyImage(imageUrl)
              res.setHeader('Content-Type', mime)
              res.setHeader('Cache-Control', 'public, max-age=86400')
              res.end(data)
            } catch (e) {
              res.statusCode = 502
              res.end('Image proxy error')
            }
            return
          }

          if (url === '/api/home') {
            try {
              const resp = await tlsRequest('GET', '/')
              const sections = parseHomepage(resp.body)
              rewriteThumbnailsInSections(sections)
              res.setHeader('Content-Type', 'application/json')
              res.end(JSON.stringify({ code: 0, data: sections }))
            } catch (e) {
              res.setHeader('Content-Type', 'application/json')
              res.end(JSON.stringify({ code: -1, message: (e as Error).message }))
            }
            return
          }

          if (url.startsWith('/api/list')) {
            try {
              const urlObj = new URL(url, 'http://localhost')
              const cat = urlObj.searchParams.get('cat') || '3'
              const page = urlObj.searchParams.get('page') || '0'
              const resp = await tlsRequest('GET', `/listinfo-${cat}-${page}.html`)
              const result = parseListPage(resp.body)
              rewriteThumbnailsInItems(result.items)
              res.setHeader('Content-Type', 'application/json')
              res.end(JSON.stringify({ code: 0, data: result }))
            } catch (e) {
              res.setHeader('Content-Type', 'application/json')
              res.end(JSON.stringify({ code: -1, message: (e as Error).message }))
            }
            return
          }

          if (url === '/api/search') {
            try {
              const chunks: Buffer[] = []
              for await (const chunk of req) {
                chunks.push(chunk)
              }
              const reqBody = Buffer.concat(chunks).toString()
              const params = new URLSearchParams(reqBody)

              const keyword = params.get('keyword') || ''
              const show = params.get('show') || 'title,text,keyboard,ftitle'

              if (show === 'tags') {
                const encodedKeyword = encodeURIComponent(keyword)
                const firstResp = await tlsRequest('GET', `/tags-${encodedKeyword}-0.html`)
                let firstResult = parseSearchResults(firstResp.body)
                if (firstResult.items.length === 0) {
                  firstResult = { items: parseListPage(firstResp.body).items, pagination: firstResult.pagination, pageUrlTemplate: null }
                }
                const allItems: ComicItem[] = [...firstResult.items]

                const maxPage = extractTagSearchMaxPage(firstResp.body, keyword)
                if (maxPage > 0) {
                  const CONCURRENCY = 32
                  for (let i = 1; i <= maxPage; i += CONCURRENCY) {
                    const batch = []
                    for (let j = i; j < i + CONCURRENCY && j <= maxPage; j++) {
                      batch.push(
                        tlsRequest('GET', `/tags-${encodedKeyword}-${j}.html`, undefined, undefined, 5)
                          .then((resp) => {
                            let result = parseSearchResults(resp.body)
                            if (result.items.length === 0) {
                              result = { items: parseListPage(resp.body).items, pagination: result.pagination, pageUrlTemplate: null }
                            }
                            return result
                          })
                          .catch(() => null)
                      )
                    }
                    const results = await Promise.all(batch)
                    for (const r of results) {
                      if (r) allItems.push(...r.items)
                    }
                  }
                }

                const result: SearchResult = {
                  items: allItems,
                  pagination: { current: 0, total: 0, hasNext: false, hasPrev: false },
                  pageUrlTemplate: null,
                }
                rewriteThumbnailsInItems(result.items)
                res.setHeader('Content-Type', 'application/json')
                res.end(JSON.stringify({ code: 0, data: result }))
                return
              }

              const searchBody = new URLSearchParams()
              searchBody.set('classid', params.get('classid') || '9')
              searchBody.set('keyboard', keyword)
              searchBody.set('show', show)
              searchBody.set('tempid', '1')
              searchBody.set('Submit', '')

              const firstResp = await tlsRequest(
                'POST',
                '/e/search/index.php',
                undefined,
                searchBody.toString(),
              )
              const firstResult = parseSearchResults(firstResp.body)
              const allItems: ComicItem[] = [...firstResult.items]
              const cookies = firstResp.setCookies

              if (firstResult.pageUrlTemplate && firstResult.pagination.total > 0) {
                const pageCount = firstResult.pagination.total + 1
                const CONCURRENCY = 32
                for (let i = 2; i <= pageCount; i += CONCURRENCY) {
                  const batch = []
                  for (let j = i; j < i + CONCURRENCY && j <= pageCount; j++) {
                    const pageUrl = firstResult.pageUrlTemplate.replace('{}', String(j))
                    batch.push(
                      tlsRequest('GET', pageUrl, undefined, undefined, 5, cookies)
                        .then((resp) => parseSearchResults(resp.body))
                        .catch(() => null)
                    )
                  }
                  const results = await Promise.all(batch)
                  for (const r of results) {
                    if (r) allItems.push(...r.items)
                  }
                }
              }

              const result: SearchResult = {
                items: allItems,
                pagination: { current: 0, total: 0, hasNext: false, hasPrev: false },
                pageUrlTemplate: null,
              }
              rewriteThumbnailsInItems(result.items)
              res.setHeader('Content-Type', 'application/json')
              res.end(JSON.stringify({ code: 0, data: result }))
            } catch (e) {
              res.setHeader('Content-Type', 'application/json')
              res.end(JSON.stringify({ code: -1, message: (e as Error).message }))
            }
            return
          }

          if (url.startsWith('/api/comic')) {
            try {
              const urlObj = new URL(url, 'http://localhost')
              const cat = urlObj.searchParams.get('cat') || '3'
              const id = urlObj.searchParams.get('id')
              if (!id) {
                res.setHeader('Content-Type', 'application/json')
                res.end(JSON.stringify({ code: -1, message: 'Missing id parameter' }))
                return
              }

              const detailResp = await tlsRequest('GET', `/moehome-${cat}-${id}.html`)
              const detail = parseComicDetail(detailResp.body, Number(cat), id)
              if (detail.thumbnail) {
                detail.thumbnail = proxyThumbnail(detail.thumbnail)
              }

              const imagesResp = await tlsRequest('GET', `/moeupup-${cat}-${id}.html`)
              const images = parseComicImages(imagesResp.body)
              detail.images = images.map(proxyThumbnail)

              res.setHeader('Content-Type', 'application/json')
              res.end(JSON.stringify({ code: 0, data: detail }))
            } catch (e) {
              res.setHeader('Content-Type', 'application/json')
              res.end(JSON.stringify({ code: -1, message: (e as Error).message }))
            }
            return
          }

          next()
        })
      },
    },
  ],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
})
