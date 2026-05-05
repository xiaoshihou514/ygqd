<script setup lang="ts">
import { reactive, onMounted } from 'vue'
import { useRoute } from 'vue-router'
import type { SearchParams } from '@/types'

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
  { value: 'likes_asc', label: '星标升序' },
] as const

const form = reactive({
  keyword: '',
  classid: 9,
  show: 'title,text,keyboard,ftitle',
  minLikes: 0,
  sortOrder: 'default',
})

function handleSubmit() {
  if (!form.keyword.trim()) return
  emit('search', {
    keyword: form.keyword.trim(),
    classid: form.classid,
    show: form.show,
    tempid: '1',
  }, form.minLikes, form.sortOrder)
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
})
</script>

<template>
  <div class="search-form-wrapper">
    <form class="search-form" @submit.prevent="handleSubmit">
      <div class="search-input-row">
        <input
          v-model="form.keyword"
          type="text"
          class="search-input"
          placeholder="输入关键词搜索..."
          autocomplete="off"
        />
        <button type="submit" class="search-btn" title="搜索">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="11" cy="11" r="8" />
            <path d="M21 21l-4.35-4.35" />
          </svg>
          <span>搜索</span>
        </button>
      </div>

      <div class="search-options">
        <div class="option-group">
          <span class="option-label">分类</span>
          <div class="option-tags">
            <button
              v-for="cat in CATEGORIES"
              :key="cat.id"
              type="button"
              class="option-tag"
              :class="{ active: form.classid === cat.id }"
              @click="form.classid = cat.id"
            >
              {{ cat.label }}
            </button>
          </div>
        </div>

        <div class="option-group">
          <span class="option-label">范围</span>
          <div class="option-tags">
            <button
              v-for="scope in SCOPE_OPTIONS"
              :key="scope.value"
              type="button"
              class="option-tag"
              :class="{ active: form.show === scope.value }"
              @click="form.show = scope.value"
            >
              {{ scope.label }}
            </button>
          </div>
        </div>

        <div class="option-group">
          <span class="option-label">排序</span>
          <div class="option-tags">
            <button
              v-for="s in SORT_OPTIONS"
              :key="s.value"
              type="button"
              class="option-tag"
              :class="{ active: form.sortOrder === s.value }"
              @click="form.sortOrder = s.value"
            >
              {{ s.label }}
            </button>
          </div>
        </div>

        <div class="option-group">
          <span class="option-label">星标</span>
          <input
            v-model.number="form.minLikes"
            type="number"
            class="filter-input"
            placeholder="最低星标数"
            min="0"
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

.search-input-row {
  display: flex;
  gap: 10px;
  margin-bottom: 20px;
}

.search-input {
  flex: 1;
  height: 44px;
  padding: 0 var(--spacing-lg);
  border: 1px solid rgba(0, 0, 0, 0.08);
  border-radius: var(--radius-pill);
  font-size: var(--font-size-body);
  outline: none;
  transition: border-color 0.2s;
  color: var(--color-text-primary);
  background: var(--color-canvas);
}

.search-input:focus {
  border-color: var(--color-primary);
  box-shadow: 0 0 0 2px var(--color-primary-focus);
}

.search-btn {
  height: 44px;
  padding: 0 var(--spacing-lg);
  background: var(--color-primary);
  color: var(--color-on-primary);
  border-radius: var(--radius-pill);
  font-size: var(--font-size-body);
  font-weight: var(--font-weight-regular);
  display: flex;
  align-items: center;
  gap: 6px;
  transition: background 0.2s;
  white-space: nowrap;
  flex-shrink: 0;
}

.search-btn:hover {
  background: var(--color-primary-focus);
}

@media (max-width: 640px) {
  .search-btn {
    padding: 0 14px;
    gap: 4px;
  }

  .search-btn svg {
    width: 16px;
    height: 16px;
  }

  .search-btn span {
    font-size: 14px;
  }
}

@media (max-width: 480px) {
  .search-btn {
    padding: 0 12px;
    min-width: 44px;
    justify-content: center;
  }

  .search-btn span {
    display: none;
  }
}

.search-options {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.option-group {
  display: flex;
  align-items: flex-start;
  gap: 12px;
}

.option-label {
  font-size: var(--font-size-fine-print);
  font-weight: var(--font-weight-semibold);
  color: var(--color-text-secondary);
  padding-top: var(--spacing-xxs);
  min-width: 36px;
}

.option-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.option-tag {
  padding: 4px 14px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-pill);
  font-size: var(--font-size-fine-print);
  font-weight: var(--font-weight-regular);
  color: var(--color-text-secondary);
  background: var(--color-canvas);
  transition: all 0.2s;
}

.option-tag:hover {
  border-color: var(--color-primary);
  color: var(--color-primary);
}

.option-tag.active {
  background: var(--color-primary);
  color: var(--color-on-primary);
  border-color: var(--color-primary);
}

.filter-input {
  height: 30px;
  width: 120px;
  padding: 0 var(--spacing-sm);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-pill);
  font-size: var(--font-size-fine-print);
  outline: none;
  transition: border-color 0.2s;
  color: var(--color-text-primary);
  background: var(--color-canvas);
}

.filter-input:focus {
  border-color: var(--color-primary);
}

.filter-input::placeholder {
  color: var(--color-text-muted);
}
</style>
