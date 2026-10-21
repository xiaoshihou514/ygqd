<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { gatewayOpen, novelAdd, novelRemove, novelShelf, novelProgress, novelSearch, novelLatest } from '@/services/api'
import type { NovelHit, NovelSeries } from '@/types'
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
const hits = ref<NovelHit[]>([])
const discoverLoading = ref(false)
const discoverHint = ref('')
const searchKeyword = ref('')
const searching = ref(false)
const searchError = ref('')
const addingHitId = ref('')

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

async function openHlib() {
  try {
    await gatewayOpen('hlib')
  } catch { /* 网关创建失败忽略,用户可重试 */ }
}

/** 发现:首页最新系列(需网关已过验证/登录,失败给引导) */
async function loadDiscover() {
  if (discoverLoading.value) return
  discoverLoading.value = true
  discoverHint.value = ''
  try {
    hits.value = await novelLatest()
  } catch (e) {
    discoverHint.value = (e as Error).message || '加载失败'
  } finally {
    discoverLoading.value = false
  }
}

async function handleSearch() {
  const kw = searchKeyword.value.trim()
  if (!kw || searching.value) return
  searching.value = true
  searchError.value = ''
  discoverHint.value = ''
  try {
    hits.value = await novelSearch(kw)
    if (!hits.value.length) searchError.value = '没有匹配的系列'
  } catch (e) {
    searchError.value = (e as Error).message || '搜索失败'
  } finally {
    searching.value = false
  }
}

/** 从发现/搜索结果一键入库 */
async function addHit(hit: NovelHit) {
  if (addingHitId.value) return
  addingHitId.value = hit.id
  try {
    await novelAdd(hit.id)
    await refresh()
  } catch (e) {
    searchError.value = (e as Error).message || '添加失败'
  } finally {
    addingHitId.value = ''
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

onMounted(() => {
  void refresh()
  void loadDiscover()
})
</script>

<template>
  <div class="novel-page">
    <div class="container">
      <div class="page-heading">
        <h2 class="page-title">小说书架</h2>
        <button class="hlib-btn" @click="openHlib">登录 / 浏览 hlib</button>
      </div>

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

      <form class="add-bar" @submit.prevent="handleSearch">
        <input
          v-model="searchKeyword"
          type="search"
          class="add-input"
          placeholder="站内搜索小说(书名/作者)"
          autocomplete="off"
        >
        <button type="submit" class="add-btn" :disabled="searching">
          {{ searching ? '搜索中…' : '搜索' }}
        </button>
        <button type="button" class="add-btn ghost" :disabled="discoverLoading" @click="loadDiscover">
          {{ discoverLoading ? '加载中…' : '最新' }}
        </button>
      </form>
      <p v-if="searchError" class="add-error">{{ searchError }}</p>
      <p v-if="discoverHint" class="add-hint">{{ discoverHint }}</p>

      <div v-if="hits.length" class="hit-list">
        <div v-for="hit in hits" :key="hit.id" class="hit-row">
          <span class="hit-title">{{ hit.title }}</span>
          <button class="hit-add" :disabled="addingHitId === hit.id" @click="addHit(hit)">
            {{ addingHitId === hit.id ? '添加中…' : '+ 书架' }}
          </button>
        </div>
      </div>

      <LoadingSpinner v-if="loading" message="加载书架..." />
      <EmptyState
        v-else-if="!shelf.length"
        title="书架还是空的"
        message="点右上「登录 / 浏览 hlib」,在弹出的窗口中登录并浏览;找到喜欢的系列后复制地址栏中的链接,回到这里粘贴添加"
      >
        <button class="hlib-btn" @click="openHlib">打开 hlib</button>
      </EmptyState>
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

.page-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--spacing-md);
  margin-bottom: var(--spacing-md);
}

.hlib-btn {
  padding: 8px 16px;
  border: 1px solid var(--color-border, var(--color-hairline));
  border-radius: var(--radius-pill, 999px);
  background: transparent;
  color: var(--color-primary);
  font-size: var(--font-size-caption);
  cursor: pointer;
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

.add-btn.ghost {
  background: transparent;
  border: 1px solid var(--color-border, var(--color-hairline));
  color: var(--color-text-primary);
}

.hit-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-bottom: var(--spacing-lg);
  max-height: 42vh;
  overflow-y: auto;
}

.hit-row {
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  padding: 8px 12px;
  border: 1px solid var(--color-hairline);
  border-radius: var(--radius-md);
  background: var(--color-card-bg);
}

.hit-title {
  flex: 1;
  min-width: 0;
  font-size: var(--font-size-caption);
  color: var(--color-text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.hit-add {
  border: 1px solid var(--color-primary);
  border-radius: var(--radius-pill, 999px);
  background: transparent;
  color: var(--color-primary);
  font-size: 12px;
  padding: 4px 12px;
  cursor: pointer;
  flex-shrink: 0;
}

.hit-add:disabled {
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
