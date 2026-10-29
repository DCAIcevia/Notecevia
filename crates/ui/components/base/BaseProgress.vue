<script setup lang="ts">
// value 省略时是不确定进度：表示还在跑，但不给百分比。
const props = withDefaults(defineProps<{ value?: number; label?: string }>(), {});

const percent = () =>
  props.value === undefined ? undefined : Math.min(100, Math.max(0, props.value));
</script>

<template>
  <div
    class="progress"
    :class="{ 'progress--idle': percent() === undefined }"
    role="progressbar"
    :aria-valuenow="percent()"
    aria-valuemin="0"
    aria-valuemax="100"
    :aria-label="label"
  >
    <div class="progress__bar" :style="percent() === undefined ? undefined : { width: `${percent()}%` }" />
  </div>
</template>

<style scoped>
.progress {
  height: 4px;
  width: 100%;
  overflow: hidden;
  border-radius: var(--radius-pill);
  background: var(--color-surface-muted);
  box-shadow: var(--shadow-inset-control);
}

.progress__bar {
  height: 100%;
  border-radius: var(--radius-pill);
  background: var(--color-accent);
  transition: width var(--duration-base) var(--ease-standard);
}

.progress--idle .progress__bar {
  width: 30%;
  animation: progress-slide 1.2s var(--ease-standard) infinite;
}

@keyframes progress-slide {
  from {
    transform: translateX(-100%);
  }
  to {
    transform: translateX(340%);
  }
}

@media (prefers-reduced-motion: reduce) {
  .progress--idle .progress__bar {
    width: 100%;
    animation: none;
  }
}
</style>
