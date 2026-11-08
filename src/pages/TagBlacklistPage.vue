<script setup lang="ts">
import { errorMessage } from '@/utils/errors'
import { computed, onMounted, ref } from 'vue'
import { useTagBlacklist } from '@/composables/useTagBlacklist'
import { novelBlacklistList, novelBlacklistAdd, novelBlacklistRemove } from '@/services/api'
import type { NovelBlacklistEntry } from '@/types'
import { useTheme } from '@/composables/useTheme'
import {
  bikaLogin,
  bikaLogout,
  bikaStatus,
  diagRead,
  diagClear,
  fetchBikaAddresses,
  getBikaAddress,
  setBikaAddress,
} from '@/services/api'
import { datedJsonFilename, exportJson, type ExportResult } from '@/utils/exportJson'
import EmptyState from '@/components/EmptyState.vue'
import type { BlacklistMode } from '@/types'

const { entries, count, add, remove, updateMode } = useTagBlacklist()
const { current, setTheme } = useTheme()

// —— 小说关键词黑名单(批次 2 需求 5:子串匹配,后端过滤) ——
const novelKeywords = ref<NovelBlacklistEntry[]>([])
const novelKeywordInput = ref('')
const novelAdding = ref(false)
const novelError = ref('')

async function loadNovelBlacklist() {
  try {
    novelKeywords.value = await novelBlacklistList()
  } catch (e) {
    console.error('[novel-blacklist]', errorMessage(e))
  }
}

async function addNovelKeyword() {
  const kw = novelKeywordInput.value.trim()
  if (!kw || novelAdding.value) return
  if (novelKeywords.value.some((e) => e.keyword === kw)) {
    novelError.value = '该关键词已在黑名单中'
    return
  }
  novelAdding.value = true
  novelError.value = ''
  try {
    await novelBlacklistAdd(kw)
    novelKeywords.value = [{ keyword: kw, createdAt: Date.now() }, ...novelKeywords.value]
    novelKeywordInput.value = ''
  } catch (e) {
    novelError.value = errorMessage(e) || '添加失败'
  } finally {
    novelAdding.value = false
  }
}

async function removeNovelKeyword(kw: string) {
  try {
    await novelBlacklistRemove(kw)
    novelKeywords.value = novelKeywords.value.filter((e) => e.keyword !== kw)
  } catch (e) {
    novelError.value = errorMessage(e) || '移除失败'
  }
}

const themeOptions = [
  { value: 'light' as const, label: '明亮' },
  { value: 'dark' as const, label: '黑暗' },
  { value: 'system' as const, label: '跟随系统' },
]

const bikaEmail = ref('')
const bikaPassword = ref('')
const bikaLoggedIn = ref(false)
const bikaAccountEmail = ref('')
const bikaLoggingIn = ref(false)
const bikaError = ref('')
const bikaMessage = ref('')

const bikaAddress = ref('')
const bikaAddressOptions = ref<string[]>([])
const bikaAddressSyncing = ref(false)
const bikaAddressError = ref('')
const diagText = ref('')
const diagOpen = ref(false)

/** 可选地址 = 同步到的列表,并保留当前已设置(可能来自旧列表)的地址 */
const addressChoices = computed(() => {
  const set = new Set(bikaAddressOptions.value)
  if (bikaAddress.value) set.add(bikaAddress.value)
  return [...set]
})

onMounted(async () => {
  try {
    const status = await bikaStatus()
    bikaLoggedIn.value = status.loggedIn
    bikaAccountEmail.value = status.email
  } catch {
    // ignore status failures
  }
  try {
    bikaAddress.value = await getBikaAddress()
  } catch {
    // ignore address failures
  }
  void loadNovelBlacklist()
})

async function handleSyncAddresses() {
  if (bikaAddressSyncing.value) return
  bikaAddressSyncing.value = true
  bikaAddressError.value = ''
  try {
    const list = await fetchBikaAddresses()
    bikaAddressOptions.value = list
    if (!list.length) {
      bikaAddressError.value = '未获取到可用地址'
    } else if (!list.includes(bikaAddress.value)) {
      // 同步到新列表后默认切到第一个,避免停留在已失效的地址
      await handleSelectAddress(list[0]!)
    }
  } catch (e) {
    bikaAddressError.value = errorMessage(e) || '同步分流地址失败'
  } finally {
    bikaAddressSyncing.value = false
  }
}

async function handleSelectAddress(address: string) {
  bikaAddressError.value = ''
  try {
    await setBikaAddress(address)
    bikaAddress.value = address
  } catch (e) {
    bikaAddressError.value = errorMessage(e) || '设置分流地址失败'
  }
}

async function toggleDiag() {
  if (diagOpen.value) {
    diagOpen.value = false
    return
  }
  try {
    diagText.value = (await diagRead()) || '（诊断日志为空）'
  } catch {
    diagText.value = '（读取失败）'
  }
  diagOpen.value = true
}

async function clearDiag() {
  try {
    await diagClear()
    diagText.value = '（已清空）'
  } catch { /* 忽略 */ }
}

async function copyDiag() {
  try {
    await navigator.clipboard.writeText(diagText.value)
  } catch { /* 剪贴板不可用时忽略 */ }
}

async function handleBikaLogin() {
  if (bikaLoggingIn.value) return
  if (!bikaEmail.value.trim() || !bikaPassword.value) {
    bikaError.value = '请输入哔咔邮箱和密码'
    return
  }
  bikaLoggingIn.value = true
  bikaError.value = ''
  bikaMessage.value = ''
  try {
    const status = await bikaLogin(bikaEmail.value.trim(), bikaPassword.value)
    bikaLoggedIn.value = status.loggedIn
    bikaAccountEmail.value = status.email
    bikaEmail.value = ''
    bikaPassword.value = ''
    bikaMessage.value = '哔咔登录成功'
  } catch (e) {
    bikaError.value = errorMessage(e) || '哔咔登录失败'
  } finally {
    bikaLoggingIn.value = false
  }
}

async function handleBikaLogout() {
  bikaError.value = ''
  bikaMessage.value = ''
  try {
    await bikaLogout()
    bikaLoggedIn.value = false
    bikaAccountEmail.value = ''
    bikaMessage.value = '已退出哔咔账号'
  } catch (e) {
    bikaError.value = errorMessage(e) || '退出失败'
  }
}

const MODE_LABELS: Record<BlacklistMode, string> = {
  fuzzy: '模糊',
  exact: '精确',
  single: '单一',
}

const MODE_CYCLE: BlacklistMode[] = ['fuzzy', 'exact', 'single']

const newTag = ref('')
const inputError = ref('')
const selectedMode = ref<BlacklistMode>('fuzzy')
const exporting = ref(false)
const exportStatus = ref('')

function handleAdd() {
  const trimmed = newTag.value.trim()
  if (!trimmed) {
    inputError.value = '请输入标签名称'
    return
  }
  if (entries.value.some((e) => e.tag === trimmed)) {
    inputError.value = '该标签已在黑名单中'
    return
  }
  add(trimmed, selectedMode.value)
  newTag.value = ''
  inputError.value = ''
  selectedMode.value = 'fuzzy'
}

function handleRemove(tag: string) {
  remove(tag)
}

function handleCycleMode(tag: string, currentMode: BlacklistMode) {
  const idx = MODE_CYCLE.indexOf(currentMode)
  if (idx === -1) return
  const nextIdx = (idx + 1) % MODE_CYCLE.length
  const next = MODE_CYCLE[nextIdx]!
  updateMode(tag, next)
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter') {
    handleAdd()
  }
  inputError.value = ''
}

const EXPORT_MESSAGES: Record<ExportResult, string> = {
  shared: '已发送黑名单备份',
  downloaded: '黑名单备份已下载',
  copied: '无法保存文件，备份 JSON 已复制',
  cancelled: '',
}

async function handleExport() {
  if (exporting.value) return
  exporting.value = true
  exportStatus.value = ''
  try {
    const result = await exportJson({
      filename: datedJsonFilename('ygqd-blacklist'),
      title: 'ygqd 黑名单备份',
      data: {
        format: 'ygqd.blacklist',
        version: 1,
        exportedAt: new Date().toISOString(),
        entries: entries.value,
      },
    })
    exportStatus.value = EXPORT_MESSAGES[result]
  } catch {
    exportStatus.value = '导出失败，请稍后重试'
  } finally {
    exporting.value = false
  }
}
</script>

<template>
  <div class="blacklist-page">
    <div class="container">
      <div class="page-header">
        <h1 class="page-title">设置</h1>
        <span class="page-count">{{ count }} 个屏蔽标签</span>
        <button class="export-btn" :disabled="exporting" @click="handleExport">
          {{ exporting ? '导出中' : '导出' }}
        </button>
      </div>

      <p v-if="exportStatus" class="export-status" aria-live="polite">{{ exportStatus }}</p>

      <div class="theme-section">
        <h2 class="section-title">外观</h2>
        <div class="theme-options">
          <button
            v-for="opt in themeOptions"
            :key="opt.value"
            class="theme-btn"
            :class="{ active: current === opt.value }"
            @click="setTheme(opt.value)"
          >
            {{ opt.label }}
          </button>
        </div>
      </div>

      <div class="section-divider" />

      <div class="theme-section">
        <h2 class="section-title">哔咔账号</h2>
        <template v-if="bikaLoggedIn">
          <p class="bika-account">已登录：{{ bikaAccountEmail }}</p>
          <button class="bika-logout-btn" @click="handleBikaLogout">退出登录</button>
        </template>
        <template v-else>
          <p class="page-desc">登录后「最新」与搜索结果将混入哔咔的漫画内容。</p>
          <div class="bika-form">
            <input
              v-model="bikaEmail"
              type="email"
              class="add-input"
              placeholder="哔咔邮箱"
              autocomplete="off"
            >
            <input
              v-model="bikaPassword"
              type="password"
              class="add-input"
              placeholder="哔咔密码"
              @keydown.enter="handleBikaLogin"
            >
            <button class="bika-login-btn" :disabled="bikaLoggingIn" @click="handleBikaLogin">
              {{ bikaLoggingIn ? '登录中...' : '登录' }}
            </button>
          </div>
        </template>
        <p v-if="bikaError" class="bika-status error">{{ bikaError }}</p>
        <p v-if="bikaMessage" class="bika-status">{{ bikaMessage }}</p>

        <div class="bika-address-block">
          <h3 class="bika-address-title">分流地址</h3>
          <p class="page-desc">
            登录或加载哔咔内容失败时，切换到其他分流地址后重试。
          </p>
          <div class="theme-options">
            <button
              class="theme-btn"
              :class="{ active: bikaAddress === '' }"
              @click="handleSelectAddress('')"
            >
              默认
            </button>
            <button
              v-for="addr in addressChoices"
              :key="addr"
              class="theme-btn"
              :class="{ active: bikaAddress === addr }"
              @click="handleSelectAddress(addr)"
            >
              {{ addr }}
            </button>
            <button
              class="theme-btn bika-address-sync"
              :disabled="bikaAddressSyncing"
              @click="handleSyncAddresses"
            >
              {{ bikaAddressSyncing ? '同步中...' : '同步地址' }}
            </button>
          </div>
          <p v-if="bikaAddressError" class="bika-status error">{{ bikaAddressError }}</p>
        </div>
      </div>

      <div class="section-divider" />

      <div class="theme-section">
        <div class="page-heading">
          <h2 class="section-title">诊断日志</h2>
          <div class="heading-actions">
            <button class="action-btn" @click="toggleDiag">
              {{ diagOpen ? '收起' : '查看' }}
            </button>
            <template v-if="diagOpen">
              <button class="action-btn" @click="copyDiag">复制</button>
              <button class="action-btn" @click="clearDiag">清空</button>
            </template>
          </div>
        </div>
        <p class="page-desc">视频/小说加载失败时,这里的日志可用于定位问题。</p>
        <pre v-if="diagOpen" class="diag-text">{{ diagText }}</pre>
      </div>

      <div class="section-divider" />

      <div class="theme-section">
        <h2 class="section-title">标签黑名单</h2>
        <p class="page-desc">
          添加标签到黑名单后，两个来源中包含该标签的漫画都会被过滤隐藏。
        </p>
      </div>

      <div class="add-bar">
        <div class="input-wrap">
          <input
            v-model="newTag"
            type="text"
            class="add-input"
            placeholder="输入要屏蔽的标签名称"
            maxlength="50"
            @keydown="handleKeydown"
          />
          <span v-if="inputError" class="input-error">{{ inputError }}</span>
        </div>
        <div class="add-actions">
          <div class="mode-pills">
            <button
              v-for="m in MODE_CYCLE"
              :key="m"
              class="mode-pill"
              :class="{ active: selectedMode === m }"
              @click="selectedMode = m"
            >
              {{ MODE_LABELS[m] }}
            </button>
          </div>
          <button class="add-btn" @click="handleAdd">
            添加
          </button>
        </div>
      </div>

      <div v-if="entries.length === 0" class="empty-wrap">
        <EmptyState
          title="黑名单为空"
          message="添加标签到黑名单以开始屏蔽内容"
        />
      </div>

      <div v-else class="tag-grid">
        <div
          v-for="entry in entries"
          :key="entry.tag"
          class="tag-item"
        >
          <div class="tag-info">
            <span class="tag-name">{{ entry.tag }}</span>
            <button
              class="mode-badge"
              :title="`点击切换匹配模式（当前：${MODE_LABELS[entry.mode]}）`"
              @click="handleCycleMode(entry.tag, entry.mode)"
            >
              {{ MODE_LABELS[entry.mode] }}
            </button>
          </div>
          <button
            class="tag-remove"
            title="移出黑名单"
            @click="handleRemove(entry.tag)"
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
              <path d="M18 6L6 18M6 6l12 12" />
            </svg>
          </button>

      <h2 class="section-title">小说关键词黑名单</h2>
      <p class="page-desc">
        添加关键词到黑名单后，标题包含该关键词的小说都会被过滤隐藏（子串匹配，与漫画标签的精确匹配不同）。
      </p>

      <div class="add-bar">
        <div class="input-wrap">
          <input
            v-model="novelKeywordInput"
            type="text"
            class="filter-input"
            placeholder="屏蔽包含此关键词的小说"
            autocomplete="off"
          />
        </div>
        <button type="submit" class="filter-add-btn" :disabled="novelAdding" @click.prevent="addNovelKeyword">
          {{ novelAdding ? '添加中…' : '添加' }}
        </button>
      </div>
      <p v-if="novelError" class="add-error">{{ novelError }}</p>

      <div v-if="novelKeywords.length" class="novel-keyword-list">
        <div v-for="e in novelKeywords" :key="e.keyword" class="novel-keyword-row">
          <span class="novel-keyword-text">{{ e.keyword }}</span>
          <button type="button" class="novel-keyword-remove" title="移除" aria-label="移除" @click="removeNovelKeyword(e.keyword)">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
              <line x1="18" y1="6" x2="6" y2="18" />
              <line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        </div>
      </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.blacklist-page {
  padding-bottom: var(--spacing-xl);
}

.page-header {
  display: flex;
  align-items: baseline;
  gap: var(--spacing-sm);
  margin-bottom: var(--spacing-xs);
  flex-wrap: wrap;
}

.page-title {
  font-size: var(--font-size-headline-title);
  font-weight: var(--font-weight-bold);
  color: var(--color-text-primary);
  letter-spacing: -0.231px;
}

.page-count {
  font-size: var(--font-size-caption);
  color: var(--color-text-muted);
}

.export-btn {
  margin-left: auto;
  min-height: 36px;
  padding: 0 var(--spacing-md);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-pill);
  color: var(--color-text-secondary);
  background: var(--color-card-bg);
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-semibold);
}

.export-btn:disabled {
  opacity: 0.55;
}

.export-status {
  margin: 0 0 var(--spacing-sm);
  color: var(--color-text-muted);
  font-size: var(--font-size-caption);
}

.page-desc {
  font-size: var(--font-size-body);
  color: var(--color-text-secondary);
  margin-bottom: var(--spacing-lg);
  line-height: 1.6;
}

.add-bar {
  display: flex;
  gap: var(--spacing-sm);
  margin-bottom: var(--spacing-xl);
  flex-direction: column;
}

.input-wrap {
  width: 100%;
  position: relative;
}

.add-input {
  width: 100%;
  height: 44px;
  padding: 0 var(--spacing-md);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  font-size: var(--font-size-body);
  outline: none;
  color: var(--color-text-primary);
  background: var(--color-canvas);
  box-sizing: border-box;
  transition: border-color 0.2s;
}

.add-input:focus {
  border-color: var(--color-primary);
}

.add-input::placeholder {
  color: var(--color-text-muted);
}

.input-error {
  position: absolute;
  top: 100%;
  left: 0;
  margin-top: 4px;
  font-size: var(--font-size-caption);
  color: var(--color-error);
}

.add-actions {
  display: flex;
  gap: var(--spacing-sm);
  align-items: center;
}

.mode-pills {
  display: flex;
  gap: 4px;
}

.mode-pill {
  height: 36px;
  padding: 0 var(--spacing-md);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-full);
  background: var(--color-card-bg);
  color: var(--color-text-secondary);
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-medium);
  cursor: pointer;
  transition: all 0.2s;
}

.mode-pill.active {
  background: var(--color-accent);
  color: var(--color-on-primary);
  border-color: var(--color-accent);
}

.add-btn {
  flex-shrink: 0;
  height: 36px;
  padding: 0 var(--spacing-lg);
  background: var(--color-primary);
  color: var(--color-on-primary);
  font-size: var(--font-size-body);
  font-weight: var(--font-weight-semibold);
  border: none;
  border-radius: var(--radius-md);
  cursor: pointer;
  transition: background 0.2s;
}

.add-btn:hover {
  background: var(--color-primary-focus);
}

.empty-wrap {
  padding-top: var(--spacing-lg);
}

.tag-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
  gap: var(--spacing-sm);
}

.tag-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--spacing-sm) var(--spacing-md);
  background: var(--color-card-bg);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  transition: border-color 0.2s;
  gap: var(--spacing-sm);
}

.tag-item:hover {
  border-color: var(--color-error);
}

.tag-info {
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  min-width: 0;
  flex: 1;
}

.mode-badge {
  flex-shrink: 0;
  height: 24px;
  padding: 0 8px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-full);
  background: var(--color-canvas);
  color: var(--color-text-muted);
  font-size: 11px;
  font-weight: var(--font-weight-medium);
  cursor: pointer;
  transition: all 0.2s;
  line-height: 24px;
}

.mode-badge:hover {
  background: var(--color-accent);
  color: var(--color-on-primary);
  border-color: var(--color-accent);
}

.tag-name {
  font-size: var(--font-size-body);
  color: var(--color-text-primary);
  font-weight: var(--font-weight-medium);
  word-break: break-all;
}

.tag-remove {
  flex-shrink: 0;
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: none;
  border: none;
  border-radius: var(--radius-sm);
  color: var(--color-text-muted);
  cursor: pointer;
  transition: all 0.2s;
}

.tag-remove:hover {
  background: var(--color-error);
  color: var(--color-on-error);
}

@media (max-width: 768px) {
  .tag-grid {
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
  }

  .add-actions {
    flex-wrap: wrap;
  }

  .add-btn {
    flex: 1;
  }

  .page-title {
    font-size: var(--font-size-tagline);
  }
}

.theme-section {
  margin-bottom: var(--spacing-xl);
}

.section-title {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: var(--font-size-body);
  font-weight: var(--font-weight-semibold);
  color: var(--color-text-secondary);
  margin-bottom: var(--spacing-sm);
}

.theme-options {
  display: flex;
  gap: var(--spacing-xs);
}

.theme-btn {
  flex: 1;
  height: 40px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  background: var(--color-card-bg);
  color: var(--color-text-primary);
  font-size: var(--font-size-body);
  font-weight: var(--font-weight-medium);
  cursor: pointer;
  transition: all 0.2s;
}

.theme-btn.active {
  background: var(--color-accent);
  color: var(--color-on-primary);
  border-color: var(--color-accent);
}

.section-divider {
  height: 1px;
  background: var(--color-hairline);
  margin-bottom: var(--spacing-xl);
}

.bika-account {
  font-size: var(--font-size-body);
  color: var(--color-text-primary);
  margin-bottom: var(--spacing-sm);
}

.bika-logout-btn {
  height: 36px;
  padding: 0 var(--spacing-lg);
  border: 1px solid var(--color-primary);
  border-radius: var(--radius-pill);
  background: transparent;
  color: var(--color-primary);
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-semibold);
}

.bika-form {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-sm);
  max-width: 360px;
}

.bika-login-btn {
  height: 44px;
  border-radius: var(--radius-pill);
  background: var(--color-primary);
  color: var(--color-on-primary);
  font-size: var(--font-size-body);
  font-weight: var(--font-weight-semibold);
  transition: opacity 0.2s;
}

.bika-login-btn:disabled {
  opacity: 0.55;
}

.bika-login-btn:focus-visible {
  outline: 2px solid var(--color-primary-focus);
  outline-offset: 2px;
}

.bika-status {
  margin-top: var(--spacing-sm);
  font-size: var(--font-size-caption);
  color: var(--color-text-muted);
}

.bika-status.error {
  color: var(--color-error);
}

.bika-address-block {
  margin-top: var(--spacing-lg);
  padding-top: var(--spacing-md);
  border-top: 1px solid var(--color-divider-soft);
}

.bika-address-title {
  margin: 0 0 var(--spacing-xs);
  font-size: var(--font-size-body);
  font-weight: 600;
  color: var(--color-text);
}

.bika-address-sync:disabled {
  opacity: 0.5;
  cursor: default;
}
.heading-actions {
  display: flex;
  gap: var(--spacing-xs);
}

.action-btn {
  padding: 6px 14px;
  border: 1px solid var(--color-border, var(--color-hairline));
  border-radius: var(--radius-pill, 999px);
  background: transparent;
  color: var(--color-text-primary);
  font-size: var(--font-size-caption);
  cursor: pointer;
}

.diag-text {
  max-height: 40vh;
  overflow: auto;
  padding: var(--spacing-sm);
  background: var(--color-divider-soft, rgba(127, 127, 127, 0.1));
  border-radius: var(--radius-md);
  font-size: 12px;
  white-space: pre-wrap;
  word-break: break-all;
  color: var(--color-text-secondary);
}
</style>
