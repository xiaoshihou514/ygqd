<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { splitAndGroup } from '@/utils/split'
import type { ComicItem, SplitGroup } from '@/types'
import EmptyState from '@/components/EmptyState.vue'

const router = useRouter()
const groups = ref<SplitGroup[]>([])
const error = ref('')
const loaded = ref(false)
const sourceItems = ref<ComicItem[]>([])

onMounted(() => {
  try {
    const raw = sessionStorage.getItem('split_items')
    if (!raw) {
      error.value = '没有可用于分组的数据，请先进行搜索'
      loaded.value = true
      return
    }
    const items: ComicItem[] = JSON.parse(raw)
    if (!items || items.length === 0) {
      error.value = '搜索结果为空，无法分组'
      loaded.value = true
      return
    }
    sourceItems.value = items
    groups.value = splitAndGroup(items)
    loaded.value = true
  } catch (e) {
    error.value = '数据解析失败'
    loaded.value = true
  }
})

function goBack() {
  router.back()
}

function formatChapters(chapters: number[]): string {
  if (chapters.length === 0) return '单篇'
  const ranges: string[] = []
  let start = chapters[0]!
  let end = chapters[0]!
  for (let i = 1; i < chapters.length; i++) {
    if (chapters[i]! === end + 1) {
      end = chapters[i]!
    } else {
      ranges.push(start === end ? `${start}` : `${start}-${end}`)
      start = chapters[i]!
      end = chapters[i]!
    }
  }
  ranges.push(start === end ? `${start}` : `${start}-${end}`)
  return ranges.join(', ')
}
</script>

<template>
  <div class="split-page">
    <div class="container">
      <div class="split-header">
        <button class="back-btn" @click="goBack">
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M19 12H5M12 19l-7-7 7-7" />
          </svg>
          返回搜索
        </button>
        <div class="header-info">
          <h1 class="split-title">作品分组</h1>
          <span v-if="groups.length > 0" class="split-summary">
            共 {{ sourceItems.length }} 条结果，分为 {{ groups.length }} 个作品
          </span>
        </div>
      </div>

      <div v-if="!loaded" class="split-loading">
        <p>加载中...</p>
      </div>

      <div v-else-if="error" class="split-error">
        <EmptyState title="无法分组" :message="error" />
      </div>

      <template v-else>
        <div v-for="group in groups" :key="`${group.author}|${group.workName}`" class="group-card">
          <div class="group-header">
            <div class="group-title-row">
              <h2 class="group-work">{{ group.workName }}</h2>
              <span v-if="group.isStandalone" class="badge-standalone">单篇</span>
              <span v-else :class="['badge-complete', group.isComplete ? 'is-complete' : 'is-incomplete']">
                {{ group.isComplete ? '完本' : '未完结' }}
              </span>
              <span v-if="group.hasExtra && !group.isStandalone" class="badge-extra">有番外</span>
            </div>
            <div v-if="group.author" class="group-author">作者：{{ group.author }}</div>
          </div>

          <div class="group-body">
            <div class="group-chapters">
              <div class="chapter-row">
                <span class="chapter-label">已知章节：</span>
                <span class="chapter-value">{{ formatChapters(group.knownChapters) }}</span>
              </div>
              <div v-if="group.missingChapters.length > 0" class="chapter-row missing">
                <span class="chapter-label">缺失章节：</span>
                <span class="chapter-value">{{ formatChapters(group.missingChapters) }}</span>
              </div>
              <div v-else-if="!group.isStandalone" class="chapter-row">
                <span class="chapter-label">缺失章节：</span>
                <span class="chapter-value ok">无</span>
              </div>
            </div>

            <div class="group-items">
              <div class="items-label">包含条目（{{ group.items.length }}）：</div>
              <div class="items-list">
                <a
                  v-for="item in group.items"
                  :key="item.id"
                  :href="`/comic/${item.categoryId}/${item.id}`"
                  class="item-link"
                >
                  <img
                    v-if="item.thumbnail"
                    :src="item.thumbnail"
                    :alt="item.title"
                    class="item-thumb"
                    loading="lazy"
                    referrerpolicy="no-referrer"
                  />
                  <span class="item-title">{{ item.title }}</span>
                </a>
              </div>
            </div>
          </div>
        </div>
      </template>
    </div>
  </div>
</template>

<style scoped>
.split-page {
  padding-bottom: var(--spacing-xl);
}

.split-header {
  margin-bottom: var(--spacing-lg);
}

.back-btn {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: var(--spacing-xs) var(--spacing-md);
  margin-bottom: var(--spacing-sm);
  color: var(--color-text-secondary);
  font-size: var(--font-size-body);
  font-weight: var(--font-weight-semibold);
  background: var(--color-card-bg);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-pill);
  transition: all 0.2s;
  cursor: pointer;
}

.back-btn:hover {
  color: var(--color-primary);
  border-color: var(--color-primary);
}

.header-info {
  display: flex;
  align-items: baseline;
  gap: var(--spacing-md);
  flex-wrap: wrap;
}

.split-title {
  font-size: var(--font-size-headline-title);
  font-weight: var(--font-weight-bold);
  color: var(--color-text-primary);
  letter-spacing: -0.231px;
}

.split-summary {
  font-size: var(--font-size-fine-print);
  color: var(--color-text-muted);
}

.split-loading,
.split-error {
  padding-top: var(--spacing-xl);
}

.group-card {
  background: var(--color-card-bg);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-card);
  margin-bottom: var(--spacing-md);
  overflow: hidden;
}

.group-header {
  padding: var(--spacing-md) var(--spacing-lg);
  border-bottom: 1px solid var(--color-hairline);
  background: var(--color-divider-soft);
}

.group-title-row {
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  flex-wrap: wrap;
  margin-bottom: var(--spacing-xxs);
}

.group-work {
  font-size: var(--font-size-tagline);
  font-weight: var(--font-weight-bold);
  color: var(--color-text-primary);
}

.badge-standalone {
  padding: 1px 8px;
  background: var(--color-tag-bg);
  color: var(--color-tag-text);
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-semibold);
  border-radius: var(--radius-pill);
}

.badge-complete {
  padding: 1px 8px;
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-semibold);
  border-radius: var(--radius-pill);
}

.badge-complete.is-complete {
  background: #e6f7ed;
  color: #1a7d3a;
}

.badge-complete.is-incomplete {
  background: #fff3e0;
  color: #b85c00;
}

.badge-extra {
  padding: 1px 8px;
  background: #e8e0f0;
  color: #5a3e85;
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-semibold);
  border-radius: var(--radius-pill);
}

.group-author {
  font-size: var(--font-size-fine-print);
  color: var(--color-text-secondary);
}

.group-body {
  padding: var(--spacing-md) var(--spacing-lg);
}

.group-chapters {
  margin-bottom: var(--spacing-md);
}

.chapter-row {
  display: flex;
  gap: var(--spacing-sm);
  margin-bottom: var(--spacing-xxs);
  font-size: var(--font-size-body);
}

.chapter-row.missing .chapter-value {
  color: #d32f2f;
}

.chapter-label {
  color: var(--color-text-secondary);
  flex-shrink: 0;
  min-width: 80px;
}

.chapter-value {
  color: var(--color-text-primary);
  font-weight: var(--font-weight-semibold);
}

.chapter-value.ok {
  color: #1a7d3a;
}

.items-label {
  font-size: var(--font-size-caption);
  color: var(--color-text-muted);
  font-weight: var(--font-weight-semibold);
  margin-bottom: var(--spacing-sm);
}

.items-list {
  display: flex;
  flex-wrap: wrap;
  gap: var(--spacing-sm);
}

.item-link {
  display: flex;
  align-items: center;
  gap: var(--spacing-xs);
  padding: var(--spacing-xs) var(--spacing-sm);
  background: var(--color-divider-soft);
  border-radius: var(--radius-md);
  text-decoration: none;
  transition: background 0.15s;
  max-width: 320px;
  min-width: 0;
}

.item-link:hover {
  background: var(--color-hairline);
}

.item-thumb {
  width: 36px;
  height: 48px;
  object-fit: cover;
  border-radius: var(--radius-sm);
  flex-shrink: 0;
  background: var(--color-divider);
}

.item-title {
  font-size: var(--font-size-fine-print);
  color: var(--color-text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
