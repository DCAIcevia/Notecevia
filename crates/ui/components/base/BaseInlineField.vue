<script setup lang="ts">
// 就地命名：创建与改名共用。Enter 提交，Esc 或失焦取消，空名等同于取消。
import { onMounted, ref } from "vue";

withDefaults(defineProps<{ placeholder?: string; label?: string }>(), {
  placeholder: "",
  label: "名称",
});

const emit = defineEmits<{ submit: [value: string]; cancel: [] }>();

const value = ref("");
const input = ref<HTMLInputElement | null>(null);
const settled = ref(false);

onMounted(() => input.value?.focus());

/** 失焦与非空提交都走这里；已经结算过就不再发第二次。 */
function commit(): void {
  if (settled.value) return;
  const title = value.value.trim();
  settled.value = true;
  if (!title) {
    emit("cancel");
    return;
  }
  emit("submit", title);
}

function cancel(): void {
  if (settled.value) return;
  settled.value = true;
  emit("cancel");
}
</script>

<template>
  <input
    ref="input"
    v-model="value"
    class="inline"
    type="text"
    :placeholder="placeholder"
    :aria-label="label"
    @keydown.enter.prevent="commit"
    @keydown.esc.prevent="cancel"
    @blur="commit"
  />
</template>

<style scoped>
.inline {
  min-width: 0;
  height: var(--control-height-md);
  padding: 0 var(--space-2);
  border: 1px solid var(--color-line-strong);
  border-radius: var(--radius-md);
  background: var(--color-surface-content);
  color: var(--color-text);
  font: inherit;
  font-size: var(--text-base);
}

.inline:focus-visible {
  border-color: var(--color-accent);
  outline: none;
}
</style>
