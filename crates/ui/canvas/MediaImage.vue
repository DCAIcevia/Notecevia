<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from "vue";
import { mediaUrl } from "./media";

const props = defineProps<{
  hash: string;
  name: string;
  fit: "cover" | "contain" | "fill";
}>();

const url = ref<string | null>(null);
const failed = ref(false);
let alive = true;

async function load(hash: string, name: string) {
  url.value = null;
  failed.value = false;
  try {
    const resolved = await mediaUrl(hash, name);
    if (alive) url.value = resolved;
  } catch {
    if (alive) failed.value = true;
  }
}

watch(() => [props.hash, props.name], () => load(props.hash, props.name), { immediate: true });

onBeforeUnmount(() => {
  alive = false;
});
</script>

<template>
  <img v-if="url" class="media" :src="url" :alt="name" :style="{ objectFit: fit }" draggable="false" />
  <div v-else class="media media--waiting">
    <span v-if="failed" class="media__note">图片读取失败</span>
  </div>
</template>

<style scoped>
.media {
  width: 100%;
  height: 100%;
  display: block;
}

.media--waiting {
  background: var(--color-surface-muted);
  border-radius: var(--radius-sm);
  display: flex;
  align-items: center;
  justify-content: center;
}

.media__note {
  font-size: var(--text-sm);
  color: var(--color-text-muted);
}
</style>
