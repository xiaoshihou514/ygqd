<script setup lang="ts">
import { reactive, ref, computed, watch, onMounted, nextTick } from 'vue'
import { useRoute } from 'vue-router'
import type { SearchParams } from '@/types'
import { useSearchHistory } from '@/composables/useSearchHistory'

const emit = defineEmits<{
  search: [params: SearchParams, minLikes: number, sortOrder: string]
}>()

const route = useRoute()

const { suggestions, remove } = useSearchHistory()

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
  classid: 9,
  show: 'title,text,keyboard,ftitle',
  minLikes: 0,
  sortOrder: 'default',
})

const inputRef = ref<HTMLInputElement | null>(null)
const showSuggestions = ref(false)
const selectedIndex = ref(-1)

const matchedSuggestions = computed(() => suggestions(form.keyword))

function selectSuggestion(keyword: string) {
  form.keyword = keyword
  showSuggestions.value = false
  selectedIndex.value = -1
  nextTick(() => {
    handleSubmit()
  })
}

function onInputFocus() {
  if (matchedSuggestions.value.length > 0) {
    showSuggestions.value = true
  }
}

function onInputBlur() {
  setTimeout(() => {
    showSuggestions.value = false
    selectedIndex.value = -1
  }, 150)
}

function onKeydown(e: KeyboardEvent) {
  const list = matchedSuggestions.value
  if (list.length === 0) return

  if (e.key === 'ArrowDown') {
    e.preventDefault()
    selectedIndex.value = Math.min(selectedIndex.value + 1, list.length - 1)
  } else if (e.key === 'ArrowUp') {
    e.preventDefault()
    selectedIndex.value = Math.max(selectedIndex.value - 1, -1)
  } else if (e.key === 'Enter' && selectedIndex.value >= 0) {
    e.preventDefault()
    selectSuggestion(list[selectedIndex.value]!)
  } else if (e.key === 'Escape') {
    showSuggestions.value = false
    selectedIndex.value = -1
  }
}

watch(() => form.keyword, () => {
  if (showSuggestions.value) {
    selectedIndex.value = -1
  }
})

watch(() => route.query.keyword, (kw) => {
  if (kw && typeof kw === 'string') form.keyword = kw
})

function clearKeyword() {
  form.keyword = ''
  showSuggestions.value = false
  selectedIndex.value = -1
  nextTick(() => {
    inputRef.value?.focus()
  })
}

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
        <div class="search-input-wrap">
          <input
            ref="inputRef"
            v-model="form.keyword"
            type="text"
            class="search-input"
            placeholder="输入关键词搜索..."
            autocomplete="off"
            @focus="onInputFocus"
            @blur="onInputBlur"
            @keydown="onKeydown"
          />
          <button
            v-if="form.keyword"
            type="button"
            class="search-clear"
            title="清空搜索"
            aria-label="清空搜索"
            @mousedown.prevent
            @click="clearKeyword"
          >
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round">
              <line x1="18" y1="6" x2="6" y2="18" />
              <line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
          <ul v-if="showSuggestions && matchedSuggestions.length > 0" class="suggestions-dropdown">
            <li
              v-for="(s, i) in matchedSuggestions"
              :key="s"
              class="suggestion-item"
              :class="{ highlighted: i === selectedIndex }"
              @mousedown.prevent="selectSuggestion(s)"
            >
              <svg class="suggestion-icon" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <polyline points="15 18 9 12 15 6" />
              </svg>
              <span class="suggestion-text">{{ s }}</span>
              <button
                type="button"
                class="suggestion-remove"
                @mousedown.stop.prevent="remove(s)"
                title="删除历史"
              >
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
                  <line x1="18" y1="6" x2="6" y2="18" />
                  <line x1="6" y1="6" x2="18" y2="18" />
                </svg>
              </button>
            </li>
          </ul>
        </div>
        <button type="submit" class="search-btn">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="11" cy="11" r="8" />
            <path d="M21 21l-4.35-4.35" />
          </svg>
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

.search-input-wrap {
  flex: 1;
  position: relative;
}

.search-input {
  width: 100%;
  height: 44px;
  padding: 0 44px 0 var(--spacing-lg);
  border: 1px solid rgba(0, 0, 0, 0.08);
  border-radius: var(--radius-pill);
  font-size: var(--font-size-body);
  outline: none;
  transition: border-color 0.2s;
  color: var(--color-text-primary);
  background: var(--color-canvas);
  box-sizing: border-box;
}

.search-input:focus {
  border-color: var(--color-primary);
  box-shadow: 0 0 0 2px var(--color-primary-focus);
}

.search-clear {
  position: absolute;
  right: 8px;
  top: 50%;
  transform: translateY(-50%);
  width: 28px;
  height: 28px;
  border: none;
  border-radius: 50%;
  background: color-mix(in srgb, var(--color-text-muted) 14%, transparent);
  color: var(--color-text-muted);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: background 0.15s, color 0.15s;
}

.search-clear:hover {
  background: color-mix(in srgb, var(--color-text-muted) 24%, transparent);
  color: var(--color-text-primary);
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

.suggestions-dropdown {
  position: absolute;
  top: 100%;
  left: 0;
  right: 0;
  margin-top: 4px;
  background: var(--color-card-bg);
  border: 1px solid var(--color-hairline);
  border-radius: var(--radius-md);
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.12);
  z-index: 100;
  overflow: hidden;
  padding: 4px 0;
  list-style: none;
}

.suggestion-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px var(--spacing-md);
  font-size: var(--font-size-body);
  color: var(--color-text-primary);
  cursor: pointer;
  transition: background 0.15s;
}

.suggestion-item:hover,
.suggestion-item.highlighted {
  background: var(--color-primary-focus);
}

.suggestion-icon {
  flex-shrink: 0;
  color: var(--color-text-muted);
  opacity: 0.5;
}

.suggestion-text {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.suggestion-remove {
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  width: 24px;
  height: 24px;
  border-radius: 50%;
  background: transparent;
  color: var(--color-text-muted);
  cursor: pointer;
  opacity: 0;
  transition: opacity 0.15s, background 0.15s;
}

.suggestion-item:hover .suggestion-remove {
  opacity: 0.7;
}

.suggestion-remove:hover {
  opacity: 1 !important;
  background: rgba(0, 0, 0, 0.08);
}
</style>
