<script setup lang="ts">
import { reactive, watch, onMounted } from 'vue'
import { useRoute } from 'vue-router'
import type { SearchParams } from '@/types'
import type { SearchScope } from '@/config/sources'
import { SEARCH_SOURCE_OPTIONS, parseSearchScope } from '@/config/sources'
import SearchInput from '@/components/SearchInput.vue'

const emit = defineEmits<{
  search: [params: SearchParams, minLikes: number, sortOrder: string]
}>()

const route = useRoute()

const CATEGORIES = [
  { id: 9, label: 'A漫' },
  { id: 3, label: '本子' },
  { id: 2, label: 'CG' },
  { id: 1, label: 'COS' },
  { id: 4, label: '套图' },
  { id: 12, label: '韩漫日漫' },
  { id: 19, label: '里番' },
  { id: 20, label: '3D' },
  { id: 21, label: '同人' },
] as const

const SCOPE_OPTIONS = [
  { value: 'title,text,keyboard,ftitle', label: '全站搜索' },
  { value: 'title,text', label: '作品' },
  { value: 'writer', label: '作者' },
  { value: 'tags', label: '标签' },
] as const

const SORT_OPTIONS = [
  { value: 'default', label: '默认排序' },
  { value: 'likes_desc', label: '星标降序' },
] as const

const form = reactive({
  keyword: '',
  source: 'all' as SearchScope,
  classid: 9,
  show: 'title,text,keyboard,ftitle',
  minLikes: 0,
  sortOrder: 'default',
})

watch(() => route.query.keyword, (kw) => {
  if (kw && typeof kw === 'string') form.keyword = kw
})

/** v-model.number 在输入框清空时给出 '',负数/小数也可能被手输进来 */
function normalizedMinLikes(): number {
  const n = Math.floor(Number(form.minLikes))
  return Number.isFinite(n) && n > 0 ? n : 0
}

function handleSubmit() {
  if (!form.keyword.trim()) return
  emit('search', {
    source: form.source,
    keyword: form.keyword.trim(),
    classid: form.classid,
    show: form.show,
    tempid: '1',
  }, normalizedMinLikes(), form.sortOrder)
}

onMounted(() => {
  const keyword = route.query.keyword as string | undefined
  if (keyword) {
    form.keyword = keyword
  }
  const show = route.query.show as string | undefined
  if (show) {
    form.show = show
  }
  const source = route.query.source as string | undefined
  if (source && SEARCH_SOURCE_OPTIONS.some((option) => option.id === source)) {
    form.source = parseSearchScope(source)
  }
})
</script>

<template>
  <div class="search-form-wrapper">
    <form class="search-form" @submit.prevent="handleSubmit">
      <SearchInput
        v-model="form.keyword"
        history-key="niacg-search-history"
        placeholder="输入关键词搜索..."
        @search="handleSubmit"
      />

      <div class="search-options">
        <div class="option-group" role="group" aria-label="分类">
          <span class="option-label" aria-hidden="true">分类</span>
          <div class="option-tags">
            <button
              v-for="cat in CATEGORIES"
              :key="cat.id"
              type="button"
              class="option-tag"
              :class="{ active: form.classid === cat.id }"
              :aria-pressed="form.classid === cat.id"
              @click="form.classid = cat.id"
            >
              {{ cat.label }}
            </button>
          </div>
        </div>

        <div class="option-group" role="group" aria-label="范围">
          <span class="option-label" aria-hidden="true">范围</span>
          <div class="option-tags">
            <button
              v-for="scope in SCOPE_OPTIONS"
              :key="scope.value"
              type="button"
              class="option-tag"
              :class="{ active: form.show === scope.value }"
              :aria-pressed="form.show === scope.value"
              @click="form.show = scope.value"
            >
              {{ scope.label }}
            </button>
          </div>
        </div>

        <div class="option-group" role="group" aria-label="排序">
          <span class="option-label" aria-hidden="true">排序</span>
          <div class="option-tags">
            <button
              v-for="s in SORT_OPTIONS"
              :key="s.value"
              type="button"
              class="option-tag"
              :class="{ active: form.sortOrder === s.value }"
              :aria-pressed="form.sortOrder === s.value"
              @click="form.sortOrder = s.value"
            >
              {{ s.label }}
            </button>
          </div>
        </div>

        <div class="option-group option-group--inline">
          <label class="option-label" for="search-min-likes">星标</label>
          <input
            id="search-min-likes"
            v-model.number="form.minLikes"
            type="number"
            class="filter-input"
            placeholder="最低星标数"
            min="0"
            step="1"
            inputmode="numeric"
            enterkeyhint="search"
            @keydown.enter.prevent="handleSubmit"
          />
        </div>
      </div>
    </form>
  </div>
</template>

<style scoped>
.search-form-wrapper {
  background: var(--color-card-bg);
  border: 1px solid var(--color-hairline);
  border-radius: var(--radius-lg);
  padding: var(--spacing-lg);
  margin-bottom: var(--spacing-lg);
}

@media (max-width: 480px) {
  .search-form-wrapper {
    padding: var(--spacing-md);
  }
}

.search-options {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-sm);
}

.option-group {
  display: flex;
  align-items: flex-start;
  gap: var(--spacing-sm);
}

.option-group--inline {
  align-items: center;
}

.option-label {
  font-size: var(--font-size-fine-print);
  font-weight: var(--font-weight-semibold);
  line-height: 28px;
  color: var(--color-text-secondary);
  min-width: 36px;
  flex-shrink: 0;
}

.option-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

/* 选项胶囊:尺寸与视频 scope 的 .sort-chip 对齐;hover 仅限有指针设备,
   避免触屏点按后残留蓝边看起来像"已选中" */
.option-tag {
  min-height: 28px;
  padding: var(--spacing-xxs) 14px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-pill);
  font-size: var(--font-size-fine-print);
  font-weight: var(--font-weight-regular);
  color: var(--color-text-secondary);
  background: var(--color-canvas);
  -webkit-tap-highlight-color: transparent;
  transition:
    background-color var(--duration-base) var(--ease-standard),
    border-color var(--duration-base) var(--ease-standard),
    color var(--duration-base) var(--ease-standard),
    transform var(--duration-fast) var(--ease-standard);
}

.option-tag:active {
  transform: scale(var(--press-scale));
}

@media (hover: hover) {
  .option-tag:hover {
    border-color: var(--color-accent);
    color: var(--color-accent);
  }
}

.option-tag.active {
  background: var(--color-primary);
  color: var(--color-on-primary);
  border-color: var(--color-primary);
}

.option-tag:focus-visible {
  outline: 2px solid var(--color-primary-focus);
  outline-offset: 2px;
}

.filter-input {
  height: 32px;
  width: 128px;
  padding: 0 var(--spacing-sm);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-pill);
  font-size: var(--font-size-fine-print);
  outline: none;
  color: var(--color-text-primary);
  background: var(--color-canvas);
  transition:
    border-color var(--duration-base) var(--ease-standard),
    box-shadow var(--duration-base) var(--ease-standard);
}

.filter-input:focus {
  border-color: var(--color-accent);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--color-accent) 18%, transparent);
}

.filter-input::placeholder {
  color: var(--color-text-muted);
}
</style>
