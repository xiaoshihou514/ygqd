<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref } from 'vue'
import { errorMessage } from '@/utils/errors'
import { useTagBlacklist } from '@/composables/useTagBlacklist'
import { useFollowedAuthors } from '@/composables/useFollowedAuthors'
import { useTheme, type Theme } from '@/composables/useTheme'
import {
  bikaLogin,
  bikaLogout,
  bikaStatus,
  diagClear,
  diagRead,
  fetchBikaAddresses,
  getBikaAddress,
  novelFollowedAuthorsReadonly,
  setBikaAddress,
  videoFollowList,
} from '@/services/api'
import { datedJsonFilename, exportJson, type ExportResult } from '@/utils/exportJson'
import type { BlacklistMode } from '@/types'

const { entries, count, add, remove, updateMode } = useTagBlacklist()
const { current, setTheme } = useTheme()

/** 成功提示自动淡出的停留时长;错误提示常驻到下一次操作 */
const FLASH_MS = 2400
/** 危险操作「再点一次确认」的有效窗口 */
const CONFIRM_MS = 3000

// ---------- 分组状态提示(统一:成功淡出、错误常驻) ----------

type FlashKey = 'bika' | 'address' | 'blacklist' | 'export' | 'diag'
type FlashTone = 'success' | 'error'
interface Flash {
  text: string
  tone: FlashTone
}

const flash = reactive<Partial<Record<FlashKey, Flash>>>({})
const flashTimers = new Map<FlashKey, number>()

function clearFlash(key: FlashKey) {
  const timer = flashTimers.get(key)
  if (timer !== undefined) {
    window.clearTimeout(timer)
    flashTimers.delete(key)
  }
  delete flash[key]
}

function showFlash(key: FlashKey, text: string, tone: FlashTone = 'success') {
  clearFlash(key)
  if (!text) return
  flash[key] = { text, tone }
  if (tone === 'success') {
    flashTimers.set(
      key,
      window.setTimeout(() => {
        flashTimers.delete(key)
        delete flash[key]
      }, FLASH_MS),
    )
  }
}

// ---------- 危险操作二次确认(按钮原地变为「再次点击确认」,无原生弹窗) ----------

type ConfirmKey = 'logout' | 'diag-clear'
const armed = ref<ConfirmKey | null>(null)
let armTimer: number | undefined

function disarm() {
  if (armTimer !== undefined) window.clearTimeout(armTimer)
  armTimer = undefined
  armed.value = null
}

function confirmTap(key: ConfirmKey, action: () => Promise<void>) {
  if (armed.value === key) {
    disarm()
    void action()
    return
  }
  disarm()
  armed.value = key
  armTimer = window.setTimeout(disarm, CONFIRM_MS)
}

onBeforeUnmount(() => {
  disarm()
  flashTimers.forEach((timer) => window.clearTimeout(timer))
  flashTimers.clear()
})

// ---------- 分段控件 ----------

interface SegOption<T> {
  value: T
  label: string
}

/** 分段控件滑块定位:选项数与当前下标写入 CSS 变量,滑块只做 transform 动画 */
function segStyle<T>(options: readonly SegOption<T>[], value: T) {
  return {
    '--seg-count': options.length,
    '--seg-index': Math.max(0, options.findIndex((o) => o.value === value)),
  }
}

const themeOptions: SegOption<Theme>[] = [
  { value: 'light', label: '明亮' },
  { value: 'dark', label: '黑暗' },
  { value: 'system', label: '跟随系统' },
]

// ---------- 哔咔账号 ----------

const bikaEmail = ref('')
const bikaPassword = ref('')
const bikaLoggedIn = ref(false)
const bikaAccountEmail = ref('')
const bikaLoggingIn = ref(false)
const bikaLoggingOut = ref(false)

const canLogin = computed(() => !!bikaEmail.value.trim() && !!bikaPassword.value)

async function handleBikaLogin() {
  if (bikaLoggingIn.value) return
  if (!canLogin.value) {
    showFlash('bika', '请输入哔咔邮箱和密码', 'error')
    return
  }
  bikaLoggingIn.value = true
  clearFlash('bika')
  try {
    const status = await bikaLogin(bikaEmail.value.trim(), bikaPassword.value)
    bikaLoggedIn.value = status.loggedIn
    bikaAccountEmail.value = status.email
    bikaEmail.value = ''
    bikaPassword.value = ''
    showFlash('bika', '哔咔登录成功')
  } catch (e) {
    showFlash('bika', errorMessage(e) || '哔咔登录失败', 'error')
  } finally {
    bikaLoggingIn.value = false
  }
}

async function handleBikaLogout() {
  if (bikaLoggingOut.value) return
  bikaLoggingOut.value = true
  clearFlash('bika')
  try {
    await bikaLogout()
    bikaLoggedIn.value = false
    bikaAccountEmail.value = ''
    showFlash('bika', '已退出哔咔账号')
  } catch (e) {
    showFlash('bika', errorMessage(e) || '退出失败', 'error')
  } finally {
    bikaLoggingOut.value = false
  }
}

// ---------- 哔咔分流地址 ----------

const bikaAddress = ref('')
const bikaAddressOptions = ref<string[]>([])
const bikaAddressSyncing = ref(false)
const bikaAddressPending = ref(false)

/** 可选地址 = 「默认」+ 同步到的列表,并保留当前已设置(可能来自旧列表)的地址 */
const addressRows = computed<SegOption<string>[]>(() => {
  const set = new Set(bikaAddressOptions.value)
  if (bikaAddress.value) set.add(bikaAddress.value)
  return [{ value: '', label: '默认' }, ...[...set].map((a) => ({ value: a, label: a }))]
})

async function applyAddress(address: string): Promise<boolean> {
  try {
    await setBikaAddress(address)
    bikaAddress.value = address
    return true
  } catch (e) {
    showFlash('address', errorMessage(e) || '设置分流地址失败', 'error')
    return false
  }
}

async function handleSelectAddress(address: string) {
  if (bikaAddressPending.value || bikaAddressSyncing.value) return
  if (address === bikaAddress.value) return
  bikaAddressPending.value = true
  clearFlash('address')
  try {
    if (await applyAddress(address)) {
      showFlash('address', address ? `已切换到 ${address}` : '已恢复默认地址')
    }
  } finally {
    bikaAddressPending.value = false
  }
}

async function handleSyncAddresses() {
  if (bikaAddressSyncing.value || bikaAddressPending.value) return
  bikaAddressSyncing.value = true
  clearFlash('address')
  try {
    const list = await fetchBikaAddresses()
    bikaAddressOptions.value = list
    if (!list.length) {
      showFlash('address', '未获取到可用地址', 'error')
    } else if (list.includes(bikaAddress.value) || (await applyAddress(list[0]!))) {
      // 当前地址不在新列表时默认切到第一个,避免停留在已失效的地址
      showFlash('address', `已同步 ${list.length} 个地址`)
    }
  } catch (e) {
    showFlash('address', errorMessage(e) || '同步分流地址失败', 'error')
  } finally {
    bikaAddressSyncing.value = false
  }
}

onMounted(async () => {
  const [status, address] = await Promise.allSettled([bikaStatus(), getBikaAddress()])
  if (status.status === 'fulfilled') {
    bikaLoggedIn.value = status.value.loggedIn
    bikaAccountEmail.value = status.value.email
  }
  if (address.status === 'fulfilled') bikaAddress.value = address.value
})

// ---------- 标签黑名单 ----------

const MODE_LABELS: Record<BlacklistMode, string> = {
  fuzzy: '模糊',
  exact: '精确',
  single: '单一',
}

const MODE_CYCLE: BlacklistMode[] = ['fuzzy', 'exact', 'single']
const modeOptions: SegOption<BlacklistMode>[] = MODE_CYCLE.map((m) => ({ value: m, label: MODE_LABELS[m] }))

/** 黑名单为空时占位行的 key,用 Symbol 避免与任何标签名冲突 */
const EMPTY_ROW_KEY = Symbol('empty')

const newTag = ref('')
const selectedMode = ref<BlacklistMode>('fuzzy')
const canAdd = computed(() => !!newTag.value.trim())

function handleAdd() {
  const trimmed = newTag.value.trim()
  if (!trimmed) {
    showFlash('blacklist', '请输入标签名称', 'error')
    return
  }
  if (entries.value.some((e) => e.tag === trimmed)) {
    showFlash('blacklist', '该标签已在黑名单中', 'error')
    return
  }
  add(trimmed, selectedMode.value)
  newTag.value = ''
  selectedMode.value = 'fuzzy'
  clearFlash('blacklist')
}

function handleTagInput() {
  if (flash.blacklist?.tone === 'error') clearFlash('blacklist')
}

function handleCycleMode(tag: string, mode: BlacklistMode) {
  const idx = MODE_CYCLE.indexOf(mode)
  const next = MODE_CYCLE[(idx + 1) % MODE_CYCLE.length]
  if (next) updateMode(tag, next)
}

// ---------- 数据导出 ----------

const exporting = ref(false)

const EXPORT_MESSAGES: Record<ExportResult, string> = {
  shared: '已发送备份',
  downloaded: '备份已下载',
  copied: '无法保存文件，备份 JSON 已复制到剪贴板',
  cancelled: '',
}

async function handleExport() {
  if (exporting.value) return
  exporting.value = true
  clearFlash('export')
  try {
    // 统一导出(用户需求:关注和设置统一用设置的导出按钮导出)
    const followedApi = useFollowedAuthors()
    const [, targetsActress, targetsMaker, novelAuthors] = await Promise.all([
      followedApi.load(),
      videoFollowList('missav', 'actress').catch(() => []),
      videoFollowList('missav', 'maker').catch(() => []),
      novelFollowedAuthorsReadonly().catch(() => []),
    ])
    const result = await exportJson({
      filename: datedJsonFilename('ygqd-backup'),
      title: 'ygqd 统一备份(黑名单 + 关注数据)',
      data: {
        format: 'ygqd.backup',
        version: 1,
        exportedAt: new Date().toISOString(),
        blacklist: { entries: entries.value },
        follows: {
          comicAuthors: followedApi.followed.value,
          videoTargetsActress: targetsActress,
          videoTargetsMaker: targetsMaker,
          novelAuthors,
        },
      },
    })
    showFlash('export', EXPORT_MESSAGES[result])
  } catch (e) {
    showFlash('export', `导出失败：${errorMessage(e) || '请稍后重试'}`, 'error')
  } finally {
    exporting.value = false
  }
}

// ---------- 诊断日志 ----------

const diagText = ref('')
const diagOpen = ref(false)
const diagLoading = ref(false)

async function toggleDiag() {
  if (diagOpen.value) {
    diagOpen.value = false
    if (armed.value === 'diag-clear') disarm()
    return
  }
  if (diagLoading.value) return
  diagLoading.value = true
  try {
    diagText.value = (await diagRead()) || '（诊断日志为空）'
  } catch (e) {
    diagText.value = `（读取失败：${errorMessage(e)}）`
  } finally {
    diagLoading.value = false
  }
  diagOpen.value = true
}

async function clearDiag() {
  try {
    await diagClear()
    diagText.value = '（已清空）'
    showFlash('diag', '诊断日志已清空')
  } catch (e) {
    showFlash('diag', errorMessage(e) || '清空失败', 'error')
  }
}

async function copyDiag() {
  try {
    await navigator.clipboard.writeText(diagText.value)
    showFlash('diag', '已复制到剪贴板')
  } catch {
    showFlash('diag', '复制失败：剪贴板不可用', 'error')
  }
}
</script>

<template>
  <div class="settings-page">
    <div class="container">
      <div class="settings">
        <h1 class="page-title">设置</h1>

        <!-- 外观 -->
        <section class="group-section" aria-labelledby="settings-appearance">
          <h2 id="settings-appearance" class="group-header">外观</h2>
          <div class="group">
            <div class="row row-control">
              <div
                class="seg"
                role="radiogroup"
                aria-labelledby="settings-appearance"
                :style="segStyle(themeOptions, current)"
              >
                <span class="seg-thumb" aria-hidden="true" />
                <button
                  v-for="opt in themeOptions"
                  :key="opt.value"
                  type="button"
                  role="radio"
                  class="seg-item"
                  :class="{ active: current === opt.value }"
                  :aria-checked="current === opt.value"
                  @click="setTheme(opt.value)"
                >
                  {{ opt.label }}
                </button>
              </div>
            </div>
          </div>
        </section>

        <!-- 哔咔账号 -->
        <section class="group-section" aria-labelledby="settings-bika">
          <h2 id="settings-bika" class="group-header">哔咔账号</h2>
          <div v-if="bikaLoggedIn" class="group">
            <div class="row">
              <span class="row-label">已登录</span>
              <span class="row-value ellipsis">{{ bikaAccountEmail }}</span>
            </div>
            <button
              type="button"
              class="row row-btn destructive"
              :disabled="bikaLoggingOut"
              @click="confirmTap('logout', handleBikaLogout)"
            >
              {{ bikaLoggingOut ? '正在退出…' : armed === 'logout' ? '再次点击确认退出' : '退出登录' }}
            </button>
          </div>
          <form v-else class="group" novalidate @submit.prevent="handleBikaLogin">
            <label class="row">
              <span class="row-label row-label-fixed">邮箱</span>
              <input
                v-model="bikaEmail"
                type="email"
                class="row-input"
                placeholder="哔咔邮箱"
                autocomplete="username"
                inputmode="email"
                autocapitalize="off"
                spellcheck="false"
                enterkeyhint="next"
                :disabled="bikaLoggingIn"
              >
            </label>
            <label class="row">
              <span class="row-label row-label-fixed">密码</span>
              <input
                v-model="bikaPassword"
                type="password"
                class="row-input"
                placeholder="哔咔密码"
                autocomplete="current-password"
                enterkeyhint="go"
                :disabled="bikaLoggingIn"
              >
            </label>
            <button
              type="submit"
              class="row row-btn accent"
              :disabled="!canLogin || bikaLoggingIn"
            >
              {{ bikaLoggingIn ? '登录中…' : '登录' }}
            </button>
          </form>
          <div class="status-slot" aria-live="polite">
            <Transition name="flash" mode="out-in">
              <p v-if="flash.bika" :key="flash.bika.text" class="status" :class="flash.bika.tone">
                {{ flash.bika.text }}
              </p>
            </Transition>
          </div>
          <p v-if="!bikaLoggedIn" class="group-footer">
            登录后「最新」与搜索结果将混入哔咔的漫画内容。
          </p>
        </section>

        <!-- 哔咔分流地址 -->
        <section class="group-section" aria-labelledby="settings-address">
          <h2 id="settings-address" class="group-header">分流地址</h2>
          <div class="group">
            <div role="radiogroup" aria-labelledby="settings-address" :aria-busy="bikaAddressPending">
              <button
                v-for="opt in addressRows"
                :key="opt.value"
                type="button"
                role="radio"
                class="row row-btn"
                :aria-checked="bikaAddress === opt.value"
                @click="handleSelectAddress(opt.value)"
              >
                <span class="row-label ellipsis">{{ opt.label }}</span>
                <svg
                  class="row-check"
                  :class="{ visible: bikaAddress === opt.value }"
                  width="18"
                  height="18"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2.5"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  aria-hidden="true"
                >
                  <path d="M5 12.5l4.5 4.5L19 7.5" />
                </svg>
              </button>
            </div>
            <button
              type="button"
              class="row row-btn accent"
              :disabled="bikaAddressSyncing"
              @click="handleSyncAddresses"
            >
              {{ bikaAddressSyncing ? '同步中…' : '同步地址' }}
            </button>
          </div>
          <div class="status-slot" aria-live="polite">
            <Transition name="flash" mode="out-in">
              <p v-if="flash.address" :key="flash.address.text" class="status" :class="flash.address.tone">
                {{ flash.address.text }}
              </p>
            </Transition>
          </div>
          <p class="group-footer">登录或加载哔咔内容失败时，切换到其他分流地址后重试。</p>
        </section>

        <!-- 标签黑名单 -->
        <section class="group-section" aria-labelledby="settings-blacklist">
          <h2 id="settings-blacklist" class="group-header">
            <span>标签黑名单</span>
            <span class="group-header-aside">{{ count }} 个</span>
          </h2>
          <form class="group" novalidate @submit.prevent="handleAdd">
            <div class="row">
              <input
                v-model="newTag"
                type="text"
                class="row-input"
                placeholder="输入要屏蔽的标签名称"
                aria-label="要屏蔽的标签名称"
                maxlength="50"
                enterkeyhint="done"
                :aria-invalid="flash.blacklist?.tone === 'error'"
                @input="handleTagInput"
              >
              <button type="submit" class="row-action" :disabled="!canAdd">添加</button>
            </div>
            <div class="row row-control">
              <div
                class="seg"
                role="radiogroup"
                aria-label="匹配模式"
                :style="segStyle(modeOptions, selectedMode)"
              >
                <span class="seg-thumb" aria-hidden="true" />
                <button
                  v-for="opt in modeOptions"
                  :key="opt.value"
                  type="button"
                  role="radio"
                  class="seg-item"
                  :class="{ active: selectedMode === opt.value }"
                  :aria-checked="selectedMode === opt.value"
                  @click="selectedMode = opt.value"
                >
                  {{ opt.label }}
                </button>
              </div>
            </div>
          </form>
          <div class="status-slot" aria-live="polite">
            <Transition name="flash" mode="out-in">
              <p v-if="flash.blacklist" :key="flash.blacklist.text" class="status" :class="flash.blacklist.tone">
                {{ flash.blacklist.text }}
              </p>
            </Transition>
          </div>

          <TransitionGroup tag="div" name="row" class="group group-list">
            <div v-for="entry in entries" :key="entry.tag" class="row tag-row">
              <span class="row-label tag-name">{{ entry.tag }}</span>
              <button
                type="button"
                class="chip-btn"
                :aria-label="`匹配模式：${MODE_LABELS[entry.mode]}，点击切换`"
                @click="handleCycleMode(entry.tag, entry.mode)"
              >
                <span class="chip">{{ MODE_LABELS[entry.mode] }}</span>
              </button>
              <button
                type="button"
                class="icon-btn"
                :aria-label="`将「${entry.tag}」移出黑名单`"
                @click="remove(entry.tag)"
              >
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true">
                  <path d="M18 6L6 18M6 6l12 12" />
                </svg>
              </button>
            </div>
            <div v-if="entries.length === 0" :key="EMPTY_ROW_KEY" class="row row-empty">
              黑名单为空
            </div>
          </TransitionGroup>
          <p class="group-footer">
            两个来源中命中标签的漫画都会被过滤隐藏。模糊：标签或标题包含即屏蔽；精确：标签完全一致；单一：作品只有这一个标签时屏蔽。点击模式标记可切换。
          </p>
        </section>

        <!-- 数据 -->
        <section class="group-section" aria-labelledby="settings-data">
          <h2 id="settings-data" class="group-header">数据</h2>
          <div class="group">
            <button
              type="button"
              class="row row-btn accent"
              :disabled="exporting"
              @click="handleExport"
            >
              {{ exporting ? '正在导出…' : '导出备份' }}
            </button>
          </div>
          <div class="status-slot" aria-live="polite">
            <Transition name="flash" mode="out-in">
              <p v-if="flash.export" :key="flash.export.text" class="status" :class="flash.export.tone">
                {{ flash.export.text }}
              </p>
            </Transition>
          </div>
          <p class="group-footer">将标签黑名单与关注数据打包为一个 JSON 文件。</p>
        </section>

        <!-- 诊断日志 -->
        <section class="group-section" aria-labelledby="settings-diag">
          <h2 id="settings-diag" class="group-header">诊断日志</h2>
          <div class="group">
            <button
              type="button"
              class="row row-btn"
              :aria-expanded="diagOpen"
              aria-controls="settings-diag-body"
              :disabled="diagLoading"
              @click="toggleDiag"
            >
              <span class="row-label">{{ diagLoading ? '读取中…' : diagOpen ? '收起日志' : '查看日志' }}</span>
              <svg
                class="row-chevron"
                :class="{ open: diagOpen }"
                width="14"
                height="14"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2.5"
                stroke-linecap="round"
                stroke-linejoin="round"
                aria-hidden="true"
              >
                <path d="M9 6l6 6-6 6" />
              </svg>
            </button>
            <Transition name="expand">
              <div v-if="diagOpen" id="settings-diag-body">
                <div class="row row-control">
                  <pre class="diag-text">{{ diagText }}</pre>
                </div>
                <button type="button" class="row row-btn accent" @click="copyDiag">复制日志</button>
                <button
                  type="button"
                  class="row row-btn destructive"
                  @click="confirmTap('diag-clear', clearDiag)"
                >
                  {{ armed === 'diag-clear' ? '再次点击确认清空' : '清空日志' }}
                </button>
              </div>
            </Transition>
          </div>
          <div class="status-slot" aria-live="polite">
            <Transition name="flash" mode="out-in">
              <p v-if="flash.diag" :key="flash.diag.text" class="status" :class="flash.diag.tone">
                {{ flash.diag.text }}
              </p>
            </Transition>
          </div>
          <p class="group-footer">视频加载失败时，这里的日志可用于定位问题。</p>
        </section>
      </div>
    </div>
  </div>
</template>

<style scoped>
.settings-page {
  padding-bottom: var(--spacing-xl);
}

/* 宽屏下限制阅读宽度,手机上即满宽 */
.settings {
  max-width: 720px;
  margin: 0 auto;
}

.page-title {
  padding-top: var(--spacing-lg);
  margin-bottom: var(--spacing-lg);
  font-size: var(--font-size-headline-title);
  font-weight: var(--font-weight-bold);
  line-height: var(--line-height-heading);
  letter-spacing: var(--letter-spacing-tight);
  color: var(--color-text-primary);
}

/* ---------- 分组(iOS inset grouped) ---------- */

.group-section + .group-section {
  margin-top: var(--spacing-xl);
}

.group-header {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--spacing-sm);
  padding: 0 var(--spacing-md);
  margin-bottom: var(--spacing-xs);
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-regular);
  line-height: var(--line-height-body);
  letter-spacing: var(--letter-spacing-caption);
  color: var(--color-text-muted);
}

.group-header-aside {
  font-variant-numeric: tabular-nums;
}

.group {
  position: relative;
  overflow: hidden;
  background: var(--color-card-bg);
  border-radius: var(--radius-md);
}

.group-list + .group-footer,
.group + .group-footer {
  margin-top: var(--spacing-xs);
}

.group-footer,
.status {
  padding: 0 var(--spacing-md);
  font-size: var(--font-size-caption);
  line-height: var(--line-height-body);
  letter-spacing: var(--letter-spacing-caption);
  color: var(--color-text-muted);
}

.status-slot + .group-footer {
  margin-top: var(--spacing-xs);
}

/* 同一分组内上下两块(黑名单:添加表单 + 列表) */
.status-slot + .group {
  margin-top: var(--spacing-sm);
}

.status {
  padding-top: var(--spacing-xs);
}

.status.success {
  color: var(--color-text-secondary);
}

.status.success::before {
  content: '';
  display: inline-block;
  width: 6px;
  height: 6px;
  margin-right: var(--spacing-xs);
  vertical-align: middle;
  border-radius: var(--radius-full);
  background: var(--color-success);
}

.status.error {
  color: var(--color-error);
}

/* ---------- 行 ---------- */

.row {
  position: relative;
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  width: 100%;
  min-height: 44px;
  padding: var(--spacing-xs) var(--spacing-md);
  font-size: var(--font-size-body);
  line-height: var(--line-height-heading);
  color: var(--color-text-primary);
  text-align: left;
  background: transparent;
}

/* 行间内缩分隔线(首行除外) */
.row::before {
  content: '';
  position: absolute;
  top: 0;
  left: var(--spacing-md);
  right: 0;
  height: 1px;
  background: var(--color-border);
  pointer-events: none;
}

.group > .row:first-child::before,
.group > :first-child > .row:first-child::before {
  display: none;
}

.row-control {
  padding: var(--spacing-xs);
}

.row-label {
  flex: 1;
  min-width: 0;
}

.row-label-fixed {
  flex: 0 0 auto;
  width: 3em;
}

.row-value {
  max-width: 60%;
  color: var(--color-text-muted);
}

.ellipsis {
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.row-empty {
  justify-content: center;
  color: var(--color-text-muted);
}

/* 可点整行:按压只变底色,不跟随全局 scale(整行缩放显得晃) */
.row-btn {
  cursor: pointer;
  -webkit-tap-highlight-color: transparent;
  transition:
    background-color var(--duration-fast) var(--ease-standard),
    opacity var(--duration-base) var(--ease-standard);
}

.row-btn:active:not(:disabled) {
  transform: none;
  background: var(--color-divider-soft);
}

.row-btn:active:disabled {
  transform: none;
}

.row-btn:disabled {
  cursor: default;
  opacity: 0.5;
}

.row-btn:focus-visible,
.row-action:focus-visible,
.chip-btn:focus-visible,
.icon-btn:focus-visible,
.seg-item:focus-visible {
  outline: 2px solid var(--color-accent);
  outline-offset: -2px;
}

.row-btn.accent,
.row-btn.destructive {
  justify-content: center;
}

.row-btn.accent {
  color: var(--color-accent);
}

.row-btn.destructive {
  color: var(--color-error);
}

.row-check {
  flex-shrink: 0;
  color: var(--color-accent);
  opacity: 0;
  transform: scale(0.6);
  transition:
    opacity var(--duration-fast) var(--ease-standard),
    transform var(--duration-base) var(--ease-out);
}

.row-check.visible {
  opacity: 1;
  transform: scale(1);
}

.row-chevron {
  flex-shrink: 0;
  color: var(--color-text-muted);
  transition: transform var(--duration-base) var(--ease-standard);
}

.row-chevron.open {
  transform: rotate(90deg);
}

/* ---------- 输入 ---------- */

.row-input {
  flex: 1;
  min-width: 0;
  min-height: 28px;
  padding: 0;
  border: none;
  outline: none;
  background: transparent;
  font-size: var(--font-size-body);
  color: var(--color-text-primary);
}

.row-input::placeholder {
  color: var(--color-text-muted);
}

.row-input:disabled {
  opacity: 0.5;
}

.row-input[aria-invalid='true'] {
  caret-color: var(--color-error);
}

.row-action {
  flex-shrink: 0;
  min-width: 44px;
  min-height: 44px;
  margin: calc(-1 * var(--spacing-xs)) calc(-1 * var(--spacing-xs)) calc(-1 * var(--spacing-xs)) 0;
  padding: 0 var(--spacing-xs);
  font-size: var(--font-size-body);
  font-weight: var(--font-weight-semibold);
  color: var(--color-accent);
  transition: opacity var(--duration-fast) var(--ease-standard);
}

.row-action:active:not(:disabled) {
  transform: none;
  opacity: 0.6;
}

.row-action:disabled {
  cursor: default;
  color: var(--color-text-muted);
}

/* ---------- 分段控件(滑块 transform 过渡) ---------- */

.seg {
  position: relative;
  isolation: isolate;
  display: grid;
  grid-auto-columns: 1fr;
  grid-auto-flow: column;
  width: 100%;
  padding: 2px;
  border-radius: var(--radius-sm);
  background: var(--color-tag-bg);
}

.seg-thumb {
  position: absolute;
  top: 2px;
  bottom: 2px;
  left: 2px;
  z-index: -1;
  width: calc((100% - 4px) / var(--seg-count, 1));
  border-radius: calc(var(--radius-sm) - 2px);
  background: var(--color-accent);
  transform: translateX(calc(var(--seg-index, 0) * 100%));
  transition: transform var(--duration-slow) var(--ease-out);
}

.seg-item {
  min-width: 0;
  min-height: 40px;
  padding: 0 var(--spacing-xs);
  border-radius: calc(var(--radius-sm) - 2px);
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-medium);
  color: var(--color-text-secondary);
  -webkit-tap-highlight-color: transparent;
  transition: color var(--duration-base) var(--ease-standard);
}

.seg-item:active {
  transform: scale(var(--press-scale));
}

.seg-item.active {
  font-weight: var(--font-weight-semibold);
  color: var(--color-on-primary);
}

/* ---------- 黑名单列表 ---------- */

.tag-row {
  gap: var(--spacing-xxs);
  padding-top: 0;
  padding-bottom: 0;
  padding-right: var(--spacing-xxs);
}

.tag-name {
  padding: var(--spacing-xs) 0;
  word-break: break-all;
}

.chip-btn {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  min-height: 44px;
  padding: 0 var(--spacing-xxs);
  -webkit-tap-highlight-color: transparent;
}

.chip-btn:active {
  transform: none;
}

.chip {
  padding: 2px var(--spacing-xs);
  border-radius: var(--radius-pill);
  background: var(--color-tag-bg);
  font-size: var(--font-size-fine-print);
  font-weight: var(--font-weight-medium);
  line-height: var(--line-height-body);
  color: var(--color-text-secondary);
  transition:
    background-color var(--duration-fast) var(--ease-standard),
    color var(--duration-fast) var(--ease-standard),
    transform var(--duration-fast) var(--ease-standard);
}

.chip-btn:active .chip {
  transform: scale(var(--press-scale));
}

.icon-btn {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 44px;
  height: 44px;
  border-radius: var(--radius-full);
  color: var(--color-text-muted);
  -webkit-tap-highlight-color: transparent;
  transition: color var(--duration-fast) var(--ease-standard);
}

.icon-btn:active {
  transform: scale(var(--press-scale));
  color: var(--color-error);
}

@media (hover: hover) {
  .chip-btn:hover .chip {
    background: var(--color-accent);
    color: var(--color-on-primary);
  }

  .icon-btn:hover {
    color: var(--color-error);
  }

  .row-btn:hover:not(:disabled) {
    background: var(--color-divider-soft);
  }
}

/* ---------- 诊断日志 ---------- */

.diag-text {
  width: 100%;
  max-height: 40vh;
  overflow: auto;
  padding: var(--spacing-sm);
  border-radius: var(--radius-sm);
  background: var(--color-bg);
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: var(--font-size-fine-print);
  line-height: var(--line-height-body);
  letter-spacing: 0;
  color: var(--color-text-secondary);
  white-space: pre-wrap;
  word-break: break-all;
  user-select: text;
}

/* ---------- 过渡 ---------- */

.flash-enter-active,
.flash-leave-active {
  transition:
    opacity var(--duration-base) var(--ease-standard),
    transform var(--duration-base) var(--ease-standard);
}

.flash-enter-from,
.flash-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}

.expand-enter-active {
  transition:
    opacity var(--duration-base) var(--ease-out),
    transform var(--duration-base) var(--ease-out);
}

.expand-leave-active {
  transition: opacity var(--duration-fast) var(--ease-in);
}

.expand-enter-from {
  opacity: 0;
  transform: translateY(-8px);
}

.expand-leave-to {
  opacity: 0;
}

.row-enter-active {
  transition:
    opacity var(--duration-base) var(--ease-out),
    transform var(--duration-base) var(--ease-out);
}

.row-leave-active {
  position: absolute;
  left: 0;
  right: 0;
  transition: opacity var(--duration-fast) var(--ease-in);
}

.row-move {
  transition: transform var(--duration-base) var(--ease-standard);
}

.row-enter-from {
  opacity: 0;
  transform: translateY(-4px);
}

.row-leave-to {
  opacity: 0;
}
</style>
