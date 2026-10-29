<script setup lang="ts">
withDefaults(
  defineProps<{
    id?: string;
    placeholder?: string;
    type?: "text" | "url" | "number";
    disabled?: boolean;
    invalid?: boolean;
    error?: string;
  }>(),
  { type: "text", disabled: false, invalid: false },
);

const model = defineModel<string>({ default: "" });
</script>

<template>
  <div class="field">
    <input
      :id="id"
      v-model="model"
      class="field__input"
      :class="{ 'field__input--invalid': invalid }"
      :type="type"
      :placeholder="placeholder"
      :disabled="disabled"
      :aria-invalid="invalid || undefined"
    />
    <p v-if="error" class="field__error">{{ error }}</p>
  </div>
</template>

<style scoped>
.field {
  display: inline-flex;
  flex-direction: column;
  gap: var(--space-1);
}

.field__input {
  height: var(--control-height-md);
  min-width: 200px;
  padding: 0 var(--space-2);
  border: 1px solid var(--color-line-strong);
  border-radius: var(--radius-md);
  background: var(--color-surface-content);
  box-shadow: var(--shadow-inset-control);
  transition: border-color var(--duration-fast) var(--ease-standard);
}

.field__input:focus-visible {
  outline: none;
  border-color: var(--color-line-active);
}

.field__input::placeholder {
  color: var(--color-text-muted);
}

.field__input:disabled {
  background: var(--color-surface-muted);
  color: var(--color-text-disabled);
}

.field__input--invalid {
  border-color: var(--color-danger);
}

.field__error {
  font-size: var(--text-sm);
  color: var(--color-danger);
}
</style>