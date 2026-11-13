<script setup lang="ts">
import { useRouter } from 'vue-router'
import { splitAndGroup } from '@/utils/split'
import type { ComicItem, SplitGroup } from '@/types'
import { comicPath } from '@/utils/comicPath'
import EmptyState from '@/components/EmptyState.vue'
import ProxyImage from '@/components/ProxyImage.vue'

const router = useRouter()

/** sessionStorage 为同步读取,分组在 setup 内一次算完,首帧即出结果(无「加载中」闪烁) */
function readGroups(): { items: ComicItem[]; groups: SplitGroup[]; error: string } {
  try {
    const raw = sessionStorage.getItem('split_items')
    if (!raw) return { items: [], groups: [], error: '没有可用于分组的数据，请先进行搜索' }
    const parsed: unknown = JSON.parse(raw)
    if (!Array.isArray(parsed) || parsed.length === 0) {
      return { items: [], groups: [], error: '搜索结果为空，无法分组' }
    }
    const items = parsed as ComicItem[]
    return { items, groups: splitAndGroup(items), error: '' }
  } catch {
    return { items: [], groups: [], error: '数据解析失败' }
  }
}

const { items: sourceItems, groups, error } = readGroups()

function goBack() {
  router.back()
}

function formatChapters(chapters: number[]): string {
  if (chapters.length === 0) return '单篇'
  const ranges: string[] = []
  let start = chapters[0]!
  let end = start
  for (let i = 1; i < chapters.length; i++) {
    const ch = chapters[i]!
    if (ch === end + 1) {
      end = ch
    } else {
      ranges.push(start === end ? `${start}` : `${start}-${end}`)
      start = ch
      end = ch
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
        <button type="button" class="back-btn" @click="goBack">
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
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

      <div v-if="error" class="split-error">
        <EmptyState title="无法分组" :message="error" />
      </div>

      <template v-else>
        <section v-for="group in groups" :key="`${group.author}|${group.workName}`" class="group-card">
          <div class="group-header">
            <div class="group-title-row">
              <h2 class="group-work">{{ group.workName }}</h2>
              <span v-if="group.isStandalone" class="badge">单篇</span>
              <span v-else class="badge" :class="group.isComplete ? 'badge-complete' : 'badge-incomplete'">
                {{ group.isComplete ? '完本' : '未完结' }}
              </span>
              <span v-if="group.hasExtra && !group.isStandalone" class="badge badge-extra">有番外</span>
            </div>
            <div v-if="group.author" class="group-author">作者：{{ group.author }}</div>
          </div>

          <div class="group-body">
            <dl class="group-chapters">
              <div class="chapter-row">
                <dt class="chapter-label">已知章节：</dt>
                <dd class="chapter-value">{{ formatChapters(group.knownChapters) }}</dd>
              </div>
              <div v-if="group.missingChapters.length > 0" class="chapter-row missing">
                <dt class="chapter-label">缺失章节：</dt>
                <dd class="chapter-value">{{ formatChapters(group.missingChapters) }}</dd>
              </div>
              <div v-else-if="!group.isStandalone" class="chapter-row">
                <dt class="chapter-label">缺失章节：</dt>
                <dd class="chapter-value ok">无</dd>
              </div>
            </dl>

            <div class="group-items">
              <div class="items-label">包含条目（{{ group.items.length }}）：</div>
              <div class="items-list">
                <!-- RouterLink:原生 <a href> 在 SPA 内会整页刷新,丢失应用状态 -->
                <RouterLink
                  v-for="item in group.items"
                  :key="`${item.source}:${item.id}`"
                  :to="comicPath(item)"
                  class="item-link"
                >
                  <span class="item-thumb">
                    <ProxyImage
                      v-if="item.thumbnail"
                      :src="item.thumbnail"
                      :source="item.source"
                      :alt="item.title"
                      loading="lazy"
                    />
                  </span>
                  <span class="item-title">{{ item.title }}</span>
                </RouterLink>
              </div>
            </div>
          </div>
        </section>
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
  gap: var(--spacing-xxs);
  min-height: 44px;
  padding: var(--spacing-xs) var(--spacing-md);
  margin-bottom: var(--spacing-sm);
  color: var(--color-text-secondary);
  font-size: var(--font-size-body);
  font-weight: var(--font-weight-semibold);
  background: var(--color-card-bg);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-pill);
  cursor: pointer;
  transition:
    color var(--duration-fast) var(--ease-standard),
    border-color var(--duration-fast) var(--ease-standard),
    transform var(--duration-fast) var(--ease-standard);
}

.back-btn:active,
.item-link:active {
  transform: scale(var(--press-scale));
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
  letter-spacing: var(--letter-spacing-tight);
}

.split-summary {
  font-size: var(--font-size-fine-print);
  color: var(--color-text-muted);
}

.split-error {
  padding-top: var(--spacing-xl);
}

.group-card {
  background: var(--color-card-bg);
  border: 1px solid var(--color-hairline);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-card);
  margin-bottom: var(--spacing-md);
  overflow: hidden;
}

.group-header {
  padding: var(--spacing-md) var(--spacing-lg);
  border-bottom: 1px solid var(--color-hairline);
  background: var(--color-surface-pearl);
}

.group-title-row {
  display: flex;
  align-items: center;
  gap: var(--spacing-xs);
  flex-wrap: wrap;
  margin-bottom: var(--spacing-xxs);
}

.group-work {
  font-size: var(--font-size-tagline);
  font-weight: var(--font-weight-bold);
  line-height: var(--line-height-heading);
  color: var(--color-text-primary);
  overflow-wrap: anywhere;
}

/* ---- 状态徽标:语义色 + 同色淡底,明暗两态通用 ---- */
.badge {
  --badge-color: var(--color-tag-text);
  padding: 1px var(--spacing-xs);
  font-size: var(--font-size-fine-print);
  font-weight: var(--font-weight-semibold);
  line-height: var(--line-height-body);
  color: var(--badge-color);
  background: color-mix(in srgb, var(--badge-color) 14%, transparent);
  border-radius: var(--radius-pill);
  white-space: nowrap;
}

.badge-complete {
  --badge-color: var(--color-success);
}

.badge-incomplete {
  --badge-color: var(--color-badge-cs);
}

.badge-extra {
  --badge-color: var(--color-badge-leak);
}

.group-author {
  font-size: var(--font-size-caption);
  color: var(--color-text-secondary);
}

.group-body {
  padding: var(--spacing-md) var(--spacing-lg);
}

.group-chapters {
  margin: 0 0 var(--spacing-md);
}

.chapter-row {
  display: flex;
  gap: var(--spacing-sm);
  margin-bottom: var(--spacing-xxs);
  font-size: var(--font-size-body);
}

.chapter-label {
  color: var(--color-text-secondary);
  flex-shrink: 0;
  min-width: 80px;
}

.chapter-value {
  margin: 0;
  min-width: 0;
  color: var(--color-text-primary);
  font-weight: var(--font-weight-semibold);
  font-variant-numeric: tabular-nums;
  overflow-wrap: anywhere;
}

.chapter-row.missing .chapter-value {
  color: var(--color-error);
}

.chapter-value.ok {
  color: var(--color-success);
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
  gap: var(--spacing-xs);
}

.item-link {
  display: flex;
  align-items: center;
  gap: var(--spacing-xs);
  min-height: 44px;
  max-width: 320px;
  min-width: 0;
  padding: var(--spacing-xs) var(--spacing-sm);
  background: var(--color-tag-bg);
  border-radius: var(--radius-md);
  text-decoration: none;
  transition:
    background-color var(--duration-fast) var(--ease-standard),
    transform var(--duration-fast) var(--ease-standard);
}

/* 缩略图固定框:ProxyImage 取图前不渲染 img,固定框避免条目宽度跳动 */
.item-thumb {
  position: relative;
  width: 36px;
  height: 48px;
  flex-shrink: 0;
  overflow: hidden;
  border-radius: var(--radius-sm);
  background: var(--color-skeleton);
}

.item-thumb img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  animation: fade-in var(--duration-base) var(--ease-out);
}

.item-title {
  min-width: 0;
  font-size: var(--font-size-caption);
  color: var(--color-text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

@media (hover: hover) {
  .back-btn:hover {
    color: var(--color-primary);
    border-color: var(--color-primary);
  }

  .item-link:hover {
    background: var(--color-hairline);
  }
}

@keyframes fade-in {
  from { opacity: 0; }
  to { opacity: 1; }
}

@media (max-width: 768px) {
  .group-header,
  .group-body {
    padding: var(--spacing-sm) var(--spacing-md);
  }

  .item-link {
    max-width: 100%;
  }
}
</style>
