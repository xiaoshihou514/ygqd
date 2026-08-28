<script setup lang="ts">
import { ref } from 'vue'
import { useTagBlacklist } from '@/composables/useTagBlacklist'
import { useTheme } from '@/composables/useTheme'
import { datedJsonFilename, exportJson, type ExportResult } from '@/utils/exportJson'
import EmptyState from '@/components/EmptyState.vue'
import type { BlacklistMode } from '@/types'

const { entries, count, add, remove, updateMode } = useTagBlacklist()
const { current, setTheme } = useTheme()

const themeOptions = [
  { value: 'light' as const, label: '明亮' },
  { value: 'dark' as const, label: '黑暗' },
  { value: 'system' as const, label: '跟随系统' },
]

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
        <h1 class="page-title">标签黑名单</h1>
        <span class="page-count">{{ count }} 个屏蔽标签</span>
        <button class="export-btn" :disabled="exporting" @click="handleExport">
          {{ exporting ? '导出中' : '导出' }}
        </button>
      </div>

      <p v-if="exportStatus" class="export-status" aria-live="polite">{{ exportStatus }}</p>

      <p class="page-desc">
        添加标签到黑名单后，包含该标签的漫画将被过滤隐藏。
      </p>

      <div class="theme-section">
        <h2 class="section-title">主题模式</h2>
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
</style>
