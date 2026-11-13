<script setup lang="ts">
// 通用搜索输入组件(任务 14:三范围搜索界面统一)——
// 搜索框 + 历史下拉 + 键盘导航,漫画 SearchForm/视频/小说 scope 表单共用;
// 各范围的历史 key 独立(传入 historyKey),板块特有选项由外层表单自行附加
import { computed, ref, watch } from 'vue'
import { createSearchHistory } from '@/composables/useSearchHistory'

const props = withDefaults(
  defineProps<{
    modelValue: string
    placeholder?: string
    historyKey: string
    loading?: boolean
    submitLabel?: string
  }>(),
  {
    placeholder: '输入关键词…',
    loading: false,
    submitLabel: '搜索',
  },
)

const emit = defineEmits<{
  'update:modelValue': [value: string]
  search: [keyword: string]
}>()

const { suggestions, add, remove, clear } = createSearchHistory(props.historyKey)

const showSuggestions = ref(false)
const selectedIndex = ref(-1)
const inputEl = ref<HTMLInputElement | null>(null)
const keyword = computed({
  get: () => props.modelValue,
  set: (v: string) => emit('update:modelValue', v),
})
const matchedSuggestions = computed(() => suggestions(keyword.value))

function clearKeyword() {
  keyword.value = ''
  inputEl.value?.focus()
}

function submitSearch() {
  const kw = keyword.value.trim()
  if (!kw) return
  add(kw)
  showSuggestions.value = false
  selectedIndex.value = -1
  emit('search', kw)
}

function selectSuggestion(text: string) {
  keyword.value = text
  add(text)
  showSuggestions.value = false
  selectedIndex.value = -1
  emit('search', text)
}

function clearAllHistory() {
  clear()
  showSuggestions.value = false
  selectedIndex.value = -1
}

function onInputFocus() {
  if (suggestions('').length > 0) {
    showSuggestions.value = true
  }
}

function onInputBlur() {
  // 延迟让下拉内 mousedown 先行
  setTimeout(() => {
    showSuggestions.value = false
    selectedIndex.value = -1
  }, 150)
}

function onInputKeydown(e: KeyboardEvent) {
  const list = matchedSuggestions.value
  if (list.length === 0) return
  if (e.key === 'ArrowDown') {
    e.preventDefault()
    showSuggestions.value = true
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

function removeSuggestion(text: string) {
  remove(text)
}
</script>

<template>
  <div class="search-input-row">
    <div class="search-input-wrap">
      <input
        ref="inputEl"
        v-model="keyword"
        type="search"
        class="search-input"
        :placeholder="placeholder"
        autocomplete="off"
        @focus="onInputFocus"
        @blur="onInputBlur"
        @keydown="onInputKeydown"
        @keyup.enter="submitSearch"
      >
      <button
        v-if="keyword"
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
          <span class="suggestion-text">{{ s }}</span>
          <button
            type="button"
            class="suggestion-remove"
            title="删除历史"
            aria-label="删除历史"
            @mousedown.stop.prevent="removeSuggestion(s)"
          >
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true">
              <line x1="18" y1="6" x2="6" y2="18" />
              <line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        </li>
        <li class="suggestion-clear-row">
          <button type="button" class="suggestion-clear" @mousedown.stop.prevent="clearAllHistory()">
            清空搜索历史
          </button>
        </li>
      </ul>
    </div>
    <button type="button" class="search-submit" :disabled="loading" @click="submitSearch">
      {{ loading ? '搜索中…' : submitLabel }}
    </button>
  </div>
</template>

<style scoped>
.search-input-row {
  display: flex;
  gap: var(--spacing-xs);
  margin-bottom: var(--spacing-md);
}

.search-input-wrap {
  flex: 1;
  min-width: 0;
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
}

.search-clear {
  position: absolute;
  right: 8px;
  top: 50%;
  transform: translateY(-50%);
  width: 28px;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  border-radius: 50%;
  background: transparent;
  color: var(--color-text-muted);
  cursor: pointer;
}

.search-clear:hover {
  color: var(--color-text-primary);
  background: var(--color-surface-pearl, var(--color-divider-soft));
}

.search-input:focus {
  outline: none;
  border-color: var(--color-primary);
}

.search-submit {
  height: 44px;
  padding: 0 22px;
  border: none;
  border-radius: var(--radius-pill, 999px);
  background: var(--color-primary);
  color: var(--color-on-primary);
  font-size: var(--font-size-body);
  cursor: pointer;
  flex-shrink: 0;
}

.search-submit:disabled {
  opacity: 0.5;
  cursor: default;
}

.suggestions-dropdown {
  position: absolute;
  top: calc(100% + 4px);
  left: 0;
  right: 0;
  margin: 0;
  padding: 4px 0;
  background: var(--color-card-bg);
  border: 1px solid var(--color-hairline);
  border-radius: var(--radius-md);
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.12);
  z-index: 100;
  overflow: hidden;
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
  border: none;
  background: transparent;
  color: var(--color-text-muted);
  cursor: pointer;
  opacity: 0;
  transition: opacity 0.15s;
}

.suggestion-item:hover .suggestion-remove {
  opacity: 0.7;
}

.suggestion-clear-row {
  border-top: 1px solid var(--color-hairline);
  margin-top: 4px;
  padding: 6px var(--spacing-md);
}

.suggestion-clear {
  width: 100%;
  padding: 4px 0;
  border: none;
  background: transparent;
  color: var(--color-text-muted);
  font-size: var(--font-size-fine-print);
  cursor: pointer;
  text-align: center;
}

.suggestion-clear:hover {
  color: var(--color-text-primary);
}
</style>
