<script setup lang="ts">
import { computed } from "vue";
import { Film, Music } from "@lucide/vue";
import type { Body, Entry, Rgb } from "../api/types";
import MediaImage from "./MediaImage.vue";

const props = withDefaults(
  defineProps<{ entry: Entry; body: Body | undefined; selected?: boolean }>(),
  { selected: false },
);

function rgb(value: Rgb | null | undefined): string | undefined {
  return value ? `rgb(${value[0]}, ${value[1]}, ${value[2]})` : undefined;
}

const frame = computed(() => {
  const geometry = props.entry.geometry;
  return {
    left: `${geometry.x}px`,
    top: `${geometry.y}px`,
    width: `${geometry.width}px`,
    height: `${geometry.height}px`,
    transform: geometry.rotation ? `rotate(${geometry.rotation}rad)` : undefined,
    zIndex: props.entry.z,
  };
});
</script>

<template>
  <div class="block" :class="{ 'block--selected': selected }" :data-entry="entry.id" :style="frame">
    <div
      v-if="body?.kind === 'text'"
      class="text"
      :style="{
        fontFamily: body.font ?? undefined,
        fontSize: `${body.size ?? 16}px`,
        color: rgb(body.color) ?? 'var(--color-text)',
        background: rgb(body.background),
      }"
    >{{ body.text }}</div>

    <MediaImage
      v-else-if="body?.kind === 'image'"
      :hash="body.media"
      :name="body.name"
      :fit="body.fit"
    />

    <div
      v-else-if="body?.kind === 'table'"
      class="table"
      :style="{
        gridTemplateColumns: `repeat(${body.cols}, minmax(0, 1fr))`,
        gridTemplateRows: `repeat(${body.rows}, minmax(0, 1fr))`,
      }"
    >
      <div v-for="(cell, index) in body.cells" :key="index" class="table__cell">{{ cell }}</div>
    </div>

    <div
      v-else-if="body?.kind === 'shape'"
      class="shape"
      :class="`shape--${body.shape}`"
      :style="{
        background: body.shape === 'line' ? undefined : rgb(body.fill),
        borderColor: rgb(body.stroke),
        borderWidth: body.shape === 'line' ? 0 : `${body.strokeWidth}px`,
        borderTopWidth: body.shape === 'line' ? `${body.strokeWidth}px` : undefined,
      }"
    />

    <div
      v-else-if="body?.kind === 'highlight'"
      class="highlight"
      :style="{ background: `rgba(${body.color[0]}, ${body.color[1]}, ${body.color[2]}, ${body.opacity})` }"
    />

    <div v-else-if="body?.kind === 'media'" class="placeholder">
      <component :is="entry.kind === 'video' ? Film : Music" :size="18" :stroke-width="1.5" />
      <span class="placeholder__name">{{ body.name || "未命名" }}</span>
    </div>
  </div>
</template>

<style scoped>
.block {
  position: absolute;
  transform-origin: center center;
}

/* 选中只加一圈描边，不改几何 */
.block--selected {
  box-shadow: 0 0 0 1px var(--color-accent);
}

.text {
  width: 100%;
  height: 100%;
  overflow: hidden;
  white-space: pre-wrap;
  line-height: 1.5;
  word-break: break-word;
}

.table {
  display: grid;
  width: 100%;
  height: 100%;
  border-top: 1px solid var(--color-line-strong);
  border-left: 1px solid var(--color-line-strong);
  background: var(--color-surface-content);
}

.table__cell {
  min-width: 0;
  min-height: 0;
  padding: 2px 4px;
  overflow: hidden;
  border-right: 1px solid var(--color-line-strong);
  border-bottom: 1px solid var(--color-line-strong);
  font-size: var(--text-base);
}

.shape {
  width: 100%;
  height: 100%;
  border-style: solid;
}

.shape--ellipse {
  border-radius: 50%;
}

.shape--line {
  width: 100%;
  height: 0;
  align-self: center;
  border-top-style: solid;
}

.highlight {
  width: 100%;
  height: 100%;
  border-radius: var(--radius-sm);
}

.placeholder {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  width: 100%;
  height: 100%;
  border-radius: var(--radius-sm);
  background: var(--color-surface-muted);
  color: var(--color-text-muted);
  font-size: var(--text-sm);
}

.placeholder__name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 70%;
}
</style>
