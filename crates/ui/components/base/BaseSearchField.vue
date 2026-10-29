<script setup lang="ts">
import { onBeforeUnmount, watch } from "vue";
import { Search, X } from "../../icons";

// 输入立刻同步到 v-model；停下来一段时间才发 search，避免每个按键都触发一次查询。
const props = withDefaults(
  defineProps<{ placeholder?: string; debounce?: number; disabled?: boolean }>(),
  { placeholder: "搜索", debounce: 250, disabled: false },
);

const emit = defineEmits<{ search: [value: string] }>();
const model = defineModel<string>({ default: "" });

let timer: number | undefined;

function schedule(value: string, wait: number) {
  window.clearTimeout(timer);
  timer = window.setTimeout(() => emit("search", value), wait);
}

watch(model, (value) => schedule(value, props.debounce));

function clear() {
  model.value = "";
  window.clearTimeout(timer);
  emit("search", "");
}

onBeforeUnmount(() => window.clearTimeout(timer));
</script>

<template>
  <div class="search" :class="{ 'search--disabled': disabled }">
    <Search class="search__icon" :size="14" :stroke-width="1.75" aria-hidden="true" />
    <input
      v-model="model"
      class="search__input"
      type="search"
      :placeholder="placeholder"
      :disabled="disabled"
      :aria-label="placeholder"
    />
    <button v-if="model" class="search__clear" type="button" aria-label="清除" @click="clear">
      <X :size="13" :stroke-width="1.75" aria-hidden="true" />
    </button>
  </div>
</template>

<style scoped>
.search {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  height: var(--control-height-md);
  padding: 0 var(--space-2);
  border: 1px solid var(--color-line-strong);
  border-radius: var(--radius-md);
  background: var(--color-surface-content);
  box-shadow: var(--shadow-inset-control);
  color: var(--color-text-muted);
  min-width: 220px;
}

.search__icon,
.search__clear svg {
  width: 14px;
  height: 14px;
  flex: none;
}

.search__input {
  flex: 1;
  min-width: 0;
  height: 100%;
  border: none;
  background: none;
  color: var(--color-text);
  outline: none;
}

.search__input::placeholder {
  color: var(--color-text-muted);
}

/* 只有键盘焦点才画环：鼠标点进来不画，免得看起来像被选中。
   `:has(:focus-visible)` 让焦点提示保留给 Tab 用户。 */
.search:has(.search__input:focus-visible) {
  outline: 2px solid var(--color-focus);
  outline-offset: 2px;
}

.search__clear {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  border: none;
  border-radius: var(--radius-sm);
  background: none;
  color: var(--color-text-muted);
  cursor: pointer;
}

.search__clear:hover {
  background: var(--color-surface-hover);
  color: var(--color-text);
}

.search--disabled {
  background: var(--color-surface-muted);
  color: var(--color-text-disabled);
}
</style>