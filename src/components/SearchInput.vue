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
const dropdownOpen = computed(() => showSuggestions.value && matchedSuggestions.value.length > 0)
const listboxId = computed(() => `${props.historyKey}-listbox`)

// 候选列表随输入/删除变化,旧高亮下标可能越界(回车会选到 undefined)
watch(matchedSuggestions, () => {
  selectedIndex.value = -1
})

let blurTimer: ReturnType<typeof setTimeout> | undefined

function closeSuggestions() {
  showSuggestions.value = false
  selectedIndex.value = -1
}

/** 触屏提交后收起软键盘让结果露出;桌面保留焦点便于连续检索 */
function dismissKeyboard() {
  if (window.matchMedia?.('(pointer: coarse)').matches) inputEl.value?.blur()
}

function clearKeyword() {
  keyword.value = ''
  inputEl.value?.focus()
}

function submitSearch() {
  const kw = keyword.value.trim()
  if (!kw) return
  add(kw)
  closeSuggestions()
  dismissKeyboard()
  emit('search', kw)
}

function selectSuggestion(text: string) {
  keyword.value = text
  add(text)
  closeSuggestions()
  dismissKeyboard()
  emit('search', text)
}

function clearAllHistory() {
  clear()
  closeSuggestions()
}

function onInputFocus() {
  clearTimeout(blurTimer)
  if (suggestions('').length > 0) {
    showSuggestions.value = true
  }
}

function onInputBlur() {
  // 延迟让下拉内 mousedown 先行
  blurTimer = setTimeout(closeSuggestions, 150)
}

function onInput() {
  // Esc 收起后继续输入,重新展开匹配的历史
  showSuggestions.value = true
}

function onInputKeydown(e: KeyboardEvent) {
  // 中文输入法组字中的回车/方向键交给 IME,不触发搜索
  if (e.isComposing || e.keyCode === 229) return
  const list = matchedSuggestions.value
  if (e.key === 'Enter') {
    // 回车统一在 keydown 处理(原先 keydown 选候选 + keyup 再提交,会连发两次搜索)
    e.preventDefault()
    const picked = dropdownOpen.value && selectedIndex.value >= 0 ? list[selectedIndex.value] : undefined
    if (picked !== undefined) selectSuggestion(picked)
    else submitSearch()
  } else if (e.key === 'Escape') {
    if (dropdownOpen.value) e.preventDefault()
    closeSuggestions()
  } else if (list.length === 0) {
    return
  } else if (e.key === 'ArrowDown') {
    e.preventDefault()
    showSuggestions.value = true
    selectedIndex.value = Math.min(selectedIndex.value + 1, list.length - 1)
  } else if (e.key === 'ArrowUp') {
    e.preventDefault()
    selectedIndex.value = Math.max(selectedIndex.value - 1, -1)
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
        :aria-label="placeholder"
        role="combobox"
        aria-autocomplete="list"
        :aria-expanded="dropdownOpen"
        :aria-controls="listboxId"
        :aria-activedescendant="dropdownOpen && selectedIndex >= 0 ? `${listboxId}-${selectedIndex}` : undefined"
        inputmode="search"
        enterkeyhint="search"
        autocomplete="off"
        autocapitalize="off"
        autocorrect="off"
        spellcheck="false"
        @focus="onInputFocus"
        @blur="onInputBlur"
        @input="onInput"
        @keydown="onInputKeydown"
      >
      <Transition name="search-fade">
        <button
          v-if="keyword"
          type="button"
          class="search-clear"
          title="清空搜索"
          aria-label="清空搜索"
          @mousedown.prevent
          @click="clearKeyword"
        >
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" aria-hidden="true">
            <line x1="18" y1="6" x2="6" y2="18" />
            <line x1="6" y1="6" x2="18" y2="18" />
          </svg>
        </button>
      </Transition>
      <Transition name="search-dropdown">
        <ul
          v-if="dropdownOpen"
          :id="listboxId"
          class="suggestions-dropdown"
          role="listbox"
          aria-label="搜索历史"
        >
          <li
            v-for="(s, i) in matchedSuggestions"
            :id="`${listboxId}-${i}`"
            :key="s"
            class="suggestion-item"
            :class="{ highlighted: i === selectedIndex }"
            role="option"
            :aria-selected="i === selectedIndex"
            @mousedown.prevent="selectSuggestion(s)"
          >
            <svg class="suggestion-icon" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <path d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
            <span class="suggestion-text">{{ s }}</span>
            <button
              type="button"
              class="suggestion-remove"
              title="删除历史"
              :aria-label="`删除历史 ${s}`"
              @mousedown.stop.prevent="removeSuggestion(s)"
            >
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true">
                <line x1="18" y1="6" x2="6" y2="18" />
                <line x1="6" y1="6" x2="18" y2="18" />
              </svg>
            </button>
          </li>
          <li class="suggestion-clear-row" role="presentation">
            <button type="button" class="suggestion-clear" @mousedown.stop.prevent="clearAllHistory()">
              清空搜索历史
            </button>
          </li>
        </ul>
      </Transition>
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

/* 胶囊搜索框(DESIGN search-input):聚焦时主色描边 + 淡光环 */
.search-input {
  width: 100%;
  height: 44px;
  padding: 0 44px 0 var(--spacing-lg);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-pill);
  font-size: var(--font-size-body);
  outline: none;
  color: var(--color-text-primary);
  background: var(--color-canvas);
  box-sizing: border-box;
  -webkit-appearance: none;
  appearance: none;
  transition:
    border-color var(--duration-base) var(--ease-standard),
    box-shadow var(--duration-base) var(--ease-standard);
}

.search-input::placeholder {
  color: var(--color-text-muted);
}

/* 隐藏 WebView 原生清除叉,只保留自绘的 .search-clear(原先两个叉并存) */
.search-input::-webkit-search-cancel-button,
.search-input::-webkit-search-decoration {
  -webkit-appearance: none;
  appearance: none;
}

.search-input:focus {
  border-color: var(--color-accent);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--color-accent) 18%, transparent);
}

/* 热区 36px(栏高 44 内),视觉只是一个小叉 */
.search-clear {
  position: absolute;
  right: var(--spacing-xxs);
  top: var(--spacing-xxs);
  width: 36px;
  height: 36px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  border-radius: var(--radius-full);
  background: transparent;
  color: var(--color-text-muted);
  cursor: pointer;
  -webkit-tap-highlight-color: transparent;
  transition:
    opacity var(--duration-fast) var(--ease-standard),
    transform var(--duration-fast) var(--ease-standard);
}

.search-clear:active {
  transform: scale(var(--press-scale));
}

@media (hover: hover) {
  .search-clear:hover {
    color: var(--color-text-primary);
    background: var(--color-tag-bg);
  }
}

.search-fade-enter-active,
.search-fade-leave-active {
  transition:
    opacity var(--duration-fast) var(--ease-standard),
    transform var(--duration-fast) var(--ease-standard);
}

.search-fade-enter-from,
.search-fade-leave-to {
  opacity: 0;
  transform: scale(0.8);
}

/* 固定最小宽度:'搜索' ↔ '搜索中…' 切换时输入框不跳宽 */
.search-submit {
  height: 44px;
  min-width: 88px;
  padding: 0 var(--spacing-md);
  border: none;
  border-radius: var(--radius-pill);
  background: var(--color-primary);
  color: var(--color-on-primary);
  font-size: var(--font-size-body);
  cursor: pointer;
  flex-shrink: 0;
  white-space: nowrap;
  -webkit-tap-highlight-color: transparent;
  transition:
    transform var(--duration-fast) var(--ease-standard),
    opacity var(--duration-base) var(--ease-standard);
}

.search-submit:active:not(:disabled) {
  transform: scale(var(--press-scale));
}

.search-submit:focus-visible {
  outline: 2px solid var(--color-primary-focus);
  outline-offset: 2px;
}

.search-submit:disabled {
  opacity: 0.5;
  cursor: default;
}

/* 历史下拉:层级压在顶/底栏之下,开合为淡入 + 轻位移 */
.suggestions-dropdown {
  position: absolute;
  top: calc(100% + var(--spacing-xxs));
  left: 0;
  right: 0;
  margin: 0;
  padding: var(--spacing-xxs) 0;
  background: var(--color-card-bg);
  border: 1px solid var(--color-hairline);
  border-radius: var(--radius-md);
  box-shadow: 0 4px 16px color-mix(in srgb, var(--color-surface-black) 12%, transparent);
  z-index: calc(var(--z-nav) - 1);
  overflow: hidden;
  list-style: none;
}

.search-dropdown-enter-active {
  transition:
    opacity var(--duration-base) var(--ease-out),
    transform var(--duration-base) var(--ease-out);
}

.search-dropdown-leave-active {
  transition:
    opacity var(--duration-fast) var(--ease-in),
    transform var(--duration-fast) var(--ease-in);
}

.search-dropdown-enter-from,
.search-dropdown-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}

.suggestion-item {
  display: flex;
  align-items: center;
  gap: var(--spacing-xs);
  min-height: 44px;
  padding: 0 var(--spacing-xxs) 0 var(--spacing-md);
  font-size: var(--font-size-body);
  color: var(--color-text-primary);
  cursor: pointer;
  -webkit-tap-highlight-color: transparent;
  transition: background-color var(--duration-fast) var(--ease-standard);
}

/* 原先高亮底色是实心主色 + 深色字,几乎看不清;改为主色淡底 */
.suggestion-item.highlighted,
.suggestion-item:active {
  background: color-mix(in srgb, var(--color-accent) 10%, transparent);
}

@media (hover: hover) {
  .suggestion-item:hover {
    background: color-mix(in srgb, var(--color-accent) 10%, transparent);
  }
}

.suggestion-icon {
  flex-shrink: 0;
  color: var(--color-text-muted);
}

.suggestion-text {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 触屏没有 hover:删除钮常显(淡色);有指针设备时悬停才显 */
.suggestion-remove {
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  width: 36px;
  height: 36px;
  border-radius: var(--radius-full);
  border: none;
  background: transparent;
  color: var(--color-text-muted);
  cursor: pointer;
  opacity: 0.7;
  transition:
    opacity var(--duration-fast) var(--ease-standard),
    background-color var(--duration-fast) var(--ease-standard);
}

@media (hover: hover) {
  .suggestion-remove {
    opacity: 0;
  }

  .suggestion-item:hover .suggestion-remove,
  .suggestion-item.highlighted .suggestion-remove {
    opacity: 0.7;
  }

  .suggestion-remove:hover {
    opacity: 1;
    background: var(--color-tag-bg);
  }
}

.suggestion-remove:focus-visible {
  opacity: 1;
  outline: 2px solid var(--color-primary-focus);
  outline-offset: -2px;
}

.suggestion-clear-row {
  border-top: 1px solid var(--color-hairline);
  margin-top: var(--spacing-xxs);
  padding: 0 var(--spacing-md);
}

.suggestion-clear {
  width: 100%;
  min-height: 40px;
  border: none;
  background: transparent;
  color: var(--color-text-muted);
  font-size: var(--font-size-fine-print);
  cursor: pointer;
  text-align: center;
  transition: color var(--duration-fast) var(--ease-standard);
}

@media (hover: hover) {
  .suggestion-clear:hover {
    color: var(--color-text-primary);
  }
}
</style>
