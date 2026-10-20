<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { novelAdd, novelRemove, novelShelf, novelProgress } from '@/services/api'
import type { NovelSeries } from '@/types'
import LoadingSpinner from '@/components/LoadingSpinner.vue'
import EmptyState from '@/components/EmptyState.vue'

const router = useRouter()
const shelf = ref<NovelSeries[]>([])
const loading = ref(true)
const adding = ref(false)
const addError = ref('')
const addHint = ref('')
const input = ref('')
const progressMap = ref<Record<string, [number, number]>>({})

async function refresh() {
  loading.value = true
  try {
    shelf.value = await novelShelf()
    for (const s of shelf.value) {
      try {
        const p = await novelProgress(s.id)
        if (p) progressMap.value[s.id] = p
      } catch { /* 进度读取失败忽略 */ }
    }
  } finally {
    loading.value = false
  }
}

async function handleAdd() {
  const value = input.value.trim()
  if (!value || adding.value) return
  adding.value = true
  addError.value = ''
  addHint.value = '正在通过网关抓取系列目录…若首次使用会弹出窗口,请完成登录与安全验证'
  try {
    await novelAdd(value)
    input.value = ''
    addHint.value = '已加入书架'
    await refresh()
  } catch (e) {
    addError.value = (e as Error).message || '添加失败'
  } finally {
    adding.value = false
    setTimeout(() => { addHint.value = '' }, 3000)
  }
}

async function handleRemove(id: string) {
  try {
    await novelRemove(id)
    shelf.value = shelf.value.filter((s) => s.id !== id)
  } catch { /* 忽略 */ }
}

function open(series: NovelSeries) {
  router.push(`/novels/${series.id}`)
}

onMounted(refresh)
</script>

<template>
  <div class="novel-page">
    <div class="container">
      <h2 class="page-title">小说书架</h2>

      <form class="add-bar" @submit.prevent="handleAdd">
        <input
          v-model="input"
          type="text"
          class="add-input"
          placeholder="粘贴 hlib 系列链接(/s/xxx)或系列 ID"
          autocomplete="off"
        >
        <button type="submit" class="add-btn" :disabled="adding">
          {{ adding ? '添加中…' : '添加' }}
        </button>
      </form>
      <p v-if="addHint" class="add-hint">{{ addHint }}</p>
      <p v-if="addError" class="add-error">{{ addError }}</p>

      <LoadingSpinner v-if="loading" message="加载书架..." />
      <EmptyState
        v-else-if="!shelf.length"
        title="书架还是空的"
        message="粘贴 hlib.cc 的系列链接加入书架;首次使用请在弹出的窗口中登录并完成安全验证"
      />
      <div v-else class="shelf-list">
        <div
          v-for="s in shelf"
          :key="s.id"
          class="shelf-card"
          @click="open(s)"
        >
          <div class="shelf-main">
            <span class="shelf-title">{{ s.title || s.id }}</span>
            <span class="shelf-meta">
              {{ s.author || '未知作者' }} · {{ s.chapters.length }} 章
              <template v-if="progressMap[s.id]">
                · 读到第 {{ progressMap[s.id]![0] }} 章第 {{ progressMap[s.id]![1] }} 页
              </template>
            </span>
          </div>
          <button class="shelf-remove" title="移出书架" @click.stop="handleRemove(s.id)">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M12 21.35l-1.45-1.32C5.4 15.36 2 12.28 2 8.5 2 5.42 4.42 3 7.5 3c1.74 0 3.41.81 4.5 2.09C13.09 3.81 14.76 3 16.5 3 19.58 3 22 5.42 22 8.5c0 3.78-3.4 6.86-8.55 11.54L12 21.35z" fill="currentColor" />
            </svg>
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.novel-page {
  padding: var(--spacing-md) 0 var(--spacing-xl);
}

.page-title {
  font-size: var(--font-size-headline-title);
  font-weight: var(--font-weight-bold);
  color: var(--color-text-primary);
  margin-bottom: var(--spacing-md);
}

.add-bar {
  display: flex;
  gap: var(--spacing-xs);
  margin-bottom: var(--spacing-sm);
}

.add-input {
  flex: 1;
  min-width: 0;
  padding: 8px 14px;
  border: 1px solid var(--color-border, var(--color-hairline));
  border-radius: var(--radius-pill, 999px);
  background: var(--color-card-bg);
  color: var(--color-text-primary);
  font-size: var(--font-size-caption);
}

.add-btn {
  padding: 8px 18px;
  border: none;
  border-radius: var(--radius-pill, 999px);
  background: var(--color-primary);
  color: #fff;
  font-size: var(--font-size-caption);
  cursor: pointer;
}

.add-btn:disabled {
  opacity: 0.5;
}

.add-hint {
  font-size: var(--font-size-fine-print);
  color: var(--color-text-secondary);
  margin: 0 0 var(--spacing-sm);
}

.add-error {
  font-size: var(--font-size-fine-print);
  color: var(--color-error);
  margin: 0 0 var(--spacing-sm);
}

.shelf-list {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-sm);
}

.shelf-card {
  display: flex;
  align-items: center;
  gap: var(--spacing-md);
  padding: var(--spacing-sm) var(--spacing-md);
  background: var(--color-card-bg);
  border: 1px solid var(--color-hairline);
  border-radius: var(--radius-md);
  cursor: pointer;
}

.shelf-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.shelf-title {
  font-size: var(--font-size-body);
  font-weight: 600;
  color: var(--color-text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.shelf-meta {
  font-size: var(--font-size-fine-print);
  color: var(--color-text-secondary);
}

.shelf-remove {
  border: none;
  background: transparent;
  color: var(--color-text-secondary);
  cursor: pointer;
  padding: 6px;
  border-radius: 50%;
}

.shelf-remove:hover {
  color: var(--color-error);
}
</style>
