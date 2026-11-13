<script setup lang="ts">
// 快速拉黑弹层(从漫画卡片抽出):长按/右键卡片封面后列出条目标签,点选即加入标签黑名单。
// 挂载即入场(遮罩淡入 + 面板上滑);关闭请求先播离场动画,结束后才 emit('close')
// 让父组件卸载——对外 props/emits 不变。打开期间锁定页面滚动、Esc 关闭、焦点移入面板。
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { useTagBlacklist } from '@/composables/useTagBlacklist'

defineProps<{ title: string; tags: string[] }>()
const emit = defineEmits<{ close: [] }>()
const { add, has } = useTagBlacklist()

const visible = ref(false)
const sheetEl = ref<HTMLElement | null>(null)
let previousFocus: HTMLElement | null = null
let restoreScroll: (() => void) | null = null

function requestClose() {
  visible.value = false
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    e.preventDefault()
    requestClose()
  }
}

/** 锁定页面滚动;桌面端补偿滚动条宽度,避免背景横向跳动 */
function lockScroll(): () => void {
  const body = document.body
  const prevOverflow = body.style.overflow
  const prevPadding = body.style.paddingRight
  const scrollbar = window.innerWidth - document.documentElement.clientWidth
  body.style.overflow = 'hidden'
  if (scrollbar > 0) body.style.paddingRight = `${scrollbar}px`
  return () => {
    body.style.overflow = prevOverflow
    body.style.paddingRight = prevPadding
  }
}

onMounted(() => {
  previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null
  restoreScroll = lockScroll()
  window.addEventListener('keydown', onKeydown)
  visible.value = true
})

function onAfterEnter() {
  sheetEl.value?.focus({ preventScroll: true })
}

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeydown)
  restoreScroll?.()
  restoreScroll = null
  previousFocus?.focus?.({ preventScroll: true })
})
</script>

<template>
  <Teleport to="body">
    <Transition name="sheet" @after-enter="onAfterEnter" @after-leave="emit('close')">
      <div v-if="visible" class="blacklist-backdrop" @click.stop="requestClose">
        <section
          ref="sheetEl"
          class="blacklist-sheet"
          role="dialog"
          aria-modal="true"
          aria-label="快速拉黑"
          tabindex="-1"
          @click.stop
        >
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
          <button type="button" class="sheet-close" @click="requestClose">完成</button>
        </section>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.blacklist-backdrop {
  position: fixed;
  inset: 0;
  z-index: var(--z-sheet);
  display: flex;
  align-items: flex-end;
  justify-content: center;
  padding: var(--spacing-md) var(--spacing-md) calc(var(--spacing-md) + env(safe-area-inset-bottom, 0px));
  background: var(--color-overlay);
  overscroll-behavior: contain;
  -webkit-tap-highlight-color: transparent;
}

.blacklist-sheet {
  width: min(100%, 520px);
  max-height: min(70vh, 560px);
  overflow-y: auto;
  overscroll-behavior: contain;
  padding: 10px var(--spacing-lg) var(--spacing-lg);
  border-radius: var(--radius-lg);
  background: var(--color-card-bg);
  outline: none;
}

/* ───── 进出场:遮罩淡入淡出 + 面板自底部滑入 ───── */
.sheet-enter-active,
.sheet-leave-active {
  transition: opacity var(--duration-slow) var(--ease-out);
}

.sheet-enter-active .blacklist-sheet,
.sheet-leave-active .blacklist-sheet {
  transition: transform var(--duration-slow) var(--ease-out);
}

.sheet-leave-active,
.sheet-leave-active .blacklist-sheet {
  transition-timing-function: var(--ease-in);
}

.sheet-enter-from,
.sheet-leave-to {
  opacity: 0;
}

.sheet-enter-from .blacklist-sheet,
.sheet-leave-to .blacklist-sheet {
  transform: translateY(100%);
}

@media (prefers-reduced-motion: reduce) {
  .sheet-enter-active .blacklist-sheet,
  .sheet-leave-active .blacklist-sheet {
    transition: none;
  }

  .sheet-enter-from .blacklist-sheet,
  .sheet-leave-to .blacklist-sheet {
    transform: none;
  }
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
  margin-top: var(--spacing-xxs);
  color: var(--color-text-primary);
  font-size: var(--font-size-body);
  font-weight: var(--font-weight-semibold);
  line-height: var(--line-height-heading);
  letter-spacing: var(--letter-spacing-body);
  overflow-wrap: anywhere;
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
  max-width: 100%;
  min-height: 44px;
  padding: 0 var(--spacing-md);
  border: 1px solid var(--color-hairline);
  border-radius: var(--radius-pill);
  color: var(--color-text-primary);
  background: var(--color-canvas);
  font-size: var(--font-size-caption);
  overflow-wrap: anywhere;
  transition:
    background-color var(--duration-base) var(--ease-standard),
    color var(--duration-base) var(--ease-standard),
    transform var(--duration-fast) var(--ease-standard);
}

.sheet-tag.added {
  color: var(--color-text-muted);
  background: var(--color-divider-soft);
  cursor: default;
}

.sheet-close {
  width: 100%;
  min-height: 44px;
  margin-top: var(--spacing-lg);
  padding: 0 var(--spacing-md);
  border-radius: var(--radius-pill);
  color: var(--color-on-primary);
  background: var(--color-primary);
  font-size: var(--font-size-body);
}
</style>
