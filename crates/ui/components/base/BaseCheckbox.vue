<script setup lang="ts">
const props = withDefaults(defineProps<{ disabled?: boolean }>(), { disabled: false });

const model = defineModel<boolean>({ default: false });
</script>

<template>
  <label class="check" :class="{ 'check--disabled': props.disabled }">
    <input v-model="model" class="check__input" type="checkbox" :disabled="props.disabled" />
    <span class="check__box" aria-hidden="true">
      <svg viewBox="0 0 12 12">
        <path
          d="M2.5 6.2 5 8.7 9.5 3.5"
          fill="none"
          stroke="currentColor"
          stroke-width="1.8"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
      </svg>
    </span>
    <span class="check__label"><slot /></span>
  </label>
</template>

<style scoped>
.check {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  cursor: pointer;
}

.check--disabled {
  color: var(--color-text-disabled);
  cursor: default;
}

.check__input {
  position: absolute;
  opacity: 0;
  width: 0;
  height: 0;
}

.check__box {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 14px;
  height: 14px;
  border: 1px solid var(--color-line-strong);
  border-radius: var(--radius-sm);
  background: var(--color-surface-content);
  color: var(--color-text-on-accent);
  transition: background-color var(--duration-fast) var(--ease-standard);
}

.check__box svg {
  width: 10px;
  height: 10px;
  opacity: 0;
}

.check__input:checked + .check__box {
  border-color: var(--color-accent);
  background: var(--color-accent);
}

.check__input:checked + .check__box svg {
  opacity: 1;
}

.check__input:focus-visible + .check__box {
  outline: 2px solid var(--color-focus);
  outline-offset: 2px;
}

.check__input:disabled + .check__box {
  background: var(--color-surface-muted);
}
</style>
