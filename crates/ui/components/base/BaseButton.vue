<script setup lang="ts">
withDefaults(
  defineProps<{
    variant?: "primary" | "secondary" | "ghost" | "danger";
    size?: "md" | "sm";
    disabled?: boolean;
    type?: "button" | "submit";
  }>(),
  { variant: "secondary", size: "md", disabled: false, type: "button" },
);
</script>

<template>
  <button
    class="btn"
    :class="[`btn--${variant}`, `btn--${size}`]"
    :type="type"
    :disabled="disabled"
  >
    <slot />
  </button>
</template>

<style scoped>
.btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-1);
  height: var(--control-height-md);
  padding: 0 var(--space-3);
  border: 1px solid transparent;
  border-radius: var(--radius-md);
  font-size: var(--text-base);
  cursor: pointer;
  transition:
    background-color var(--duration-fast) var(--ease-standard),
    border-color var(--duration-fast) var(--ease-standard);
}

.btn--sm {
  height: var(--control-height-sm);
  padding: 0 var(--space-2);
  font-size: var(--text-sm);
}

.btn--secondary {
  border-color: var(--color-line-strong);
  background: var(--color-surface-content);
  box-shadow: var(--shadow-control);
}

.btn--secondary:hover:not(:disabled) {
  background: var(--color-surface-hover);
}

.btn--primary {
  border-color: var(--color-accent);
  background: var(--color-accent);
  color: var(--color-text-on-accent);
  box-shadow: var(--shadow-control);
}

.btn--primary:hover:not(:disabled) {
  border-color: var(--color-accent-hover);
  background: var(--color-accent-hover);
}

.btn--ghost {
  background: transparent;
}

.btn--ghost:hover:not(:disabled) {
  background: var(--color-surface-hover);
}

.btn--danger {
  border-color: var(--color-line-strong);
  color: var(--color-danger);
  background: var(--color-surface-content);
  box-shadow: var(--shadow-control);
}

.btn--danger:hover:not(:disabled) {
  background: var(--color-danger-soft);
}

.btn:disabled {
  border-color: var(--color-line);
  color: var(--color-text-disabled);
  background: var(--color-surface-content);
  cursor: default;
}
</style>
