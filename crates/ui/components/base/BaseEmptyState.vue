<script setup lang="ts">
// 空态必须给出下一步，否则用户停在这里。
defineProps<{ title: string; hint?: string }>();
</script>

<template>
  <div class="empty">
    <p class="empty__title">{{ title }}</p>
    <p v-if="hint" class="empty__hint">{{ hint }}</p>
    <div v-if="$slots.action" class="empty__action">
      <slot name="action" />
    </div>
  </div>
</template>

<style scoped>
.empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-12) var(--space-6);
  text-align: center;
}

.empty__title {
  font-size: var(--text-lg);
  color: var(--color-text-muted);
}

.empty__hint {
  max-width: 32em;
  font-size: var(--text-sm);
  color: var(--color-text-muted);
}

.empty__action {
  margin-top: var(--space-2);
}
</style>
