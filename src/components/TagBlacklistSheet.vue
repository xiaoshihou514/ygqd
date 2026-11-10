<script setup lang="ts">
// 快速拉黑弹层(从漫画卡片抽出):长按/右键卡片封面后列出条目标签,点选即加入标签黑名单
import { useTagBlacklist } from '@/composables/useTagBlacklist'

defineProps<{ title: string; tags: string[] }>()
const emit = defineEmits<{ close: [] }>()
const { add, has } = useTagBlacklist()
</script>

<template>
  <Teleport to="body">
    <div class="blacklist-backdrop" @click.stop="emit('close')">
      <section class="blacklist-sheet" role="dialog" aria-modal="true" aria-label="快速拉黑" @click.stop>
        <div class="sheet-handle" />
        <p class="sheet-eyebrow">快速拉黑</p>
        <h2 class="sheet-title">{{ title }}</h2>
        <p class="sheet-hint">选择不想再看到的标签</p>
        <div v-if="tags.length" class="sheet-tags">
          <button
            v-for="tag in tags"
            :key="tag"
            type="button"
            class="sheet-tag"
            :class="{ added: has(tag) }"
            :disabled="has(tag)"
            @click="add(tag)"
          >
            {{ has(tag) ? `已拉黑 · ${tag}` : tag }}
          </button>
        </div>
        <p v-else class="sheet-empty">这个条目没有可用标签</p>
        <button type="button" class="sheet-close" @click="emit('close')">完成</button>
      </section>
    </div>
  </Teleport>
</template>

<style scoped>
.blacklist-backdrop {
  position: fixed;
  inset: 0;
  z-index: 300;
  display: flex;
  align-items: flex-end;
  justify-content: center;
  padding: var(--spacing-md);
  background: rgba(0, 0, 0, 0.34);
  backdrop-filter: blur(4px);
}

.blacklist-sheet {
  width: min(100%, 520px);
  max-height: min(70vh, 560px);
  overflow-y: auto;
  padding: 10px var(--spacing-lg) calc(var(--spacing-lg) + env(safe-area-inset-bottom));
  border-radius: var(--radius-lg);
  background: var(--color-card-bg);
    animation: sheet-in 180ms ease-out;
}

.sheet-handle {
  width: 36px;
  height: 4px;
  margin: 0 auto var(--spacing-md);
  border-radius: var(--radius-pill);
  background: var(--color-hairline);
}

.sheet-eyebrow {
  color: var(--color-primary);
  font-size: var(--font-size-fine-print);
  font-weight: var(--font-weight-semibold);
}

.sheet-title {
  margin-top: 4px;
  color: var(--color-text-primary);
  font-size: var(--font-size-body);
  line-height: 1.35;
}

.sheet-hint,
.sheet-empty {
  margin-top: var(--spacing-xs);
  color: var(--color-text-muted);
  font-size: var(--font-size-caption);
}

.sheet-tags {
  display: flex;
  flex-wrap: wrap;
  gap: var(--spacing-xs);
  margin-top: var(--spacing-md);
}

.sheet-tag {
  padding: 8px 13px;
  border: 1px solid var(--color-hairline);
  border-radius: var(--radius-pill);
  color: var(--color-text-primary);
  background: var(--color-canvas);
  font-size: var(--font-size-caption);
}

.sheet-tag.added {
  color: var(--color-text-muted);
  background: var(--color-divider-soft);
}

.sheet-close {
  width: 100%;
  margin-top: var(--spacing-lg);
  padding: 11px 16px;
  border-radius: var(--radius-pill);
  color: var(--color-on-primary);
  background: var(--color-primary);
  font-size: var(--font-size-body);
}

@keyframes sheet-in {
  from { transform: translateY(20px); opacity: 0; }
  to { transform: translateY(0); opacity: 1; }
}

@media (prefers-reduced-motion: reduce) {
  .blacklist-sheet { animation: none; }
}

</style>
