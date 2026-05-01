<script setup lang="ts">
import { reactive } from 'vue'
import type { SearchParams } from '@/types'

const emit = defineEmits<{
  search: [params: SearchParams]
}>()

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

const form = reactive({
  keyword: '',
  classid: 9,
  show: 'title,text,keyboard,ftitle',
})

function handleSubmit() {
  if (!form.keyword.trim()) return
  emit('search', {
    keyword: form.keyword.trim(),
    classid: form.classid,
    show: form.show,
    tempid: '1',
  })
}
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
        <button type="submit" class="search-btn">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="11" cy="11" r="8" />
            <path d="M21 21l-4.35-4.35" />
          </svg>
          搜索
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
}

.search-btn:hover {
  background: var(--color-primary-focus);
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
</style>
