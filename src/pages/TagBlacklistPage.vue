<script setup lang="ts">
import { ref } from 'vue'
import { useTagBlacklist } from '@/composables/useTagBlacklist'
import EmptyState from '@/components/EmptyState.vue'

const { tagList, count, add, remove } = useTagBlacklist()

const newTag = ref('')
const inputError = ref('')

function handleAdd() {
  const trimmed = newTag.value.trim()
  if (!trimmed) {
    inputError.value = '请输入标签名称'
    return
  }
  if (tagList.value.includes(trimmed)) {
    inputError.value = '该标签已在黑名单中'
    return
  }
  add(trimmed)
  newTag.value = ''
  inputError.value = ''
}

function handleRemove(tag: string) {
  remove(tag)
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter') {
    handleAdd()
  }
  inputError.value = ''
}
</script>

<template>
  <div class="blacklist-page">
    <div class="container">
      <div class="page-header">
        <h1 class="page-title">标签黑名单</h1>
        <span class="page-count">{{ count }} 个屏蔽标签</span>
      </div>

      <p class="page-desc">
        添加标签到黑名单后，包含该标签的漫画将被过滤隐藏。
      </p>

      <div class="add-bar">
        <div class="input-wrap">
          <input
            v-model="newTag"
            type="text"
            class="add-input"
            placeholder="输入要屏蔽的标签名称"
            maxlength="50"
            @keydown="handleKeydown"
          />
          <span v-if="inputError" class="input-error">{{ inputError }}</span>
        </div>
        <button class="add-btn" @click="handleAdd">
          添加
        </button>
      </div>

      <div v-if="tagList.length === 0" class="empty-wrap">
        <EmptyState
          title="黑名单为空"
          message="添加标签到黑名单以开始屏蔽内容"
        />
      </div>

      <div v-else class="tag-grid">
        <div
          v-for="tag in tagList"
          :key="tag"
          class="tag-item"
        >
          <span class="tag-name">{{ tag }}</span>
          <button
            class="tag-remove"
            title="移出黑名单"
            @click="handleRemove(tag)"
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
              <path d="M18 6L6 18M6 6l12 12" />
            </svg>
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.blacklist-page {
  padding-bottom: var(--spacing-xl);
}

.page-header {
  display: flex;
  align-items: baseline;
  gap: var(--spacing-sm);
  margin-bottom: var(--spacing-xs);
  flex-wrap: wrap;
}

.page-title {
  font-size: var(--font-size-headline-title);
  font-weight: var(--font-weight-bold);
  color: var(--color-text-primary);
  letter-spacing: -0.231px;
}

.page-count {
  font-size: var(--font-size-caption);
  color: var(--color-text-muted);
}

.page-desc {
  font-size: var(--font-size-body);
  color: var(--color-text-secondary);
  margin-bottom: var(--spacing-lg);
  line-height: 1.6;
}

.add-bar {
  display: flex;
  gap: var(--spacing-sm);
  margin-bottom: var(--spacing-xl);
}

.input-wrap {
  flex: 1;
  min-width: 0;
  position: relative;
}

.add-input {
  width: 100%;
  height: 44px;
  padding: 0 var(--spacing-md);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  font-size: var(--font-size-body);
  outline: none;
  color: var(--color-text-primary);
  background: var(--color-canvas);
  box-sizing: border-box;
  transition: border-color 0.2s;
}

.add-input:focus {
  border-color: var(--color-primary);
}

.add-input::placeholder {
  color: var(--color-text-muted);
}

.input-error {
  position: absolute;
  top: 100%;
  left: 0;
  margin-top: 4px;
  font-size: var(--font-size-caption);
  color: var(--color-error);
}

.add-btn {
  flex-shrink: 0;
  height: 44px;
  padding: 0 var(--spacing-lg);
  background: var(--color-primary);
  color: var(--color-on-primary);
  font-size: var(--font-size-body);
  font-weight: var(--font-weight-semibold);
  border: none;
  border-radius: var(--radius-md);
  cursor: pointer;
  transition: background 0.2s;
}

.add-btn:hover {
  background: var(--color-primary-focus);
}

.empty-wrap {
  padding-top: var(--spacing-lg);
}

.tag-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
  gap: var(--spacing-sm);
}

.tag-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--spacing-sm) var(--spacing-md);
  background: var(--color-card-bg);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-md);
  transition: border-color 0.2s;
}

.tag-item:hover {
  border-color: var(--color-error);
}

.tag-name {
  font-size: var(--font-size-body);
  color: var(--color-text-primary);
  font-weight: var(--font-weight-medium);
  word-break: break-all;
}

.tag-remove {
  flex-shrink: 0;
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: none;
  border: none;
  border-radius: var(--radius-sm);
  color: var(--color-text-muted);
  cursor: pointer;
  transition: all 0.2s;
}

.tag-remove:hover {
  background: var(--color-error);
  color: var(--color-on-error);
}

@media (max-width: 768px) {
  .tag-grid {
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
  }

  .add-bar {
    flex-direction: column;
  }

  .add-btn {
    width: 100%;
  }

  .page-title {
    font-size: var(--font-size-tagline);
  }
}
</style>
