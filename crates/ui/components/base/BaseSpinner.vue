<script setup lang="ts">
withDefaults(defineProps<{ label?: string }>(), { label: "加载中" });
</script>

<template>
  <span class="spinner" role="status" :aria-label="label" />
</template>

<style scoped>
.spinner {
  display: inline-block;
  width: 14px;
  height: 14px;
  border: 2px solid var(--color-line-strong);
  border-top-color: var(--color-accent);
  border-radius: var(--radius-pill);
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

@media (prefers-reduced-motion: reduce) {
  .spinner {
    animation: none;
  }
}
</style>
