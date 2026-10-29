<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { Film, Music } from "@lucide/vue";
import type { Body, Entry, Rgb } from "../api/types";
import MediaImage from "./MediaImage.vue";

const props = withDefaults(
  defineProps<{
    entry: Entry;
    body: Body | undefined;
    selected?: boolean;
    editing?: boolean;
    zoom?: number;
  }>(),
  { selected: false, editing: false, zoom: 1 },
);

const CORNERS = ["nw", "ne", "se", "sw"] as const;

const emit = defineEmits<{ text: [value: string]; commit: [] }>();

const editor = ref<HTMLElement | null>(null);

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

// 手柄按屏幕尺寸画：层被缩放后仍保持同样大的点击区域。
const handleStyle = computed(() => {
  const size = 9 / props.zoom;
  return { width: `${size}px`, height: `${size}px`, borderWidth: `${1.5 / props.zoom}px` };
});

const textStyle = computed(() => {
  const body = props.body;
  if (body?.kind !== "text") return {};
  return {
    fontFamily: body.font ?? undefined,
    fontSize: `${body.size ?? 16}px`,
    color: rgb(body.color) ?? "var(--color-text)",
    background: rgb(body.background),
  };
});

async function focusEditor(): Promise<void> {
  await nextTick();
  const element = editor.value;
  if (!element) return;
  if (props.body?.kind === "text") element.textContent = props.body.text;
  element.focus();
  const range = document.createRange();
  range.selectNodeContents(element);
  range.collapse(false);
  const selection = window.getSelection();
  selection?.removeAllRanges();
  selection?.addRange(range);
}

watch(
  () => props.editing,
  (editing) => {
    if (editing) void focusEditor();
  },
  { immediate: true },
);

function onInput(event: Event): void {
  emit("text", (event.target as HTMLElement).innerText);
}
</script>

<template>
  <div
    class="block"
    :class="{ 'block--selected': selected, 'block--editing': editing }"
    :data-entry="entry.id"
    :style="frame"
  >
    <div
      v-if="body?.kind === 'text' && editing"
      ref="editor"
      class="text text--editing"
      :style="textStyle"
      contenteditable="true"
      spellcheck="false"
      @input="onInput"
      @blur="emit('commit')"
      @keydown.esc.prevent.stop="emit('commit')"
    />

    <div v-else-if="body?.kind === 'text'" class="text" :style="textStyle">{{ body.text }}</div>

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

    <div v-if="selected && !editing && !entry.locked" class="handles" aria-hidden="true">
      <span
        v-for="corner in CORNERS"
        :key="corner"
        class="handle"
        :class="`handle--${corner}`"
        :data-handle="corner"
        :style="handleStyle"
      />
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

.block--editing {
  box-shadow: 0 0 0 2px var(--color-accent);
  cursor: text;
}

.handles {
  position: absolute;
  inset: 0;
  pointer-events: none;
}

.handle {
  position: absolute;
  display: block;
  border-style: solid;
  border-color: var(--color-accent);
  border-radius: var(--radius-sm);
  background: var(--color-surface-content);
  pointer-events: auto;
}

.handle--nw {
  top: 0;
  left: 0;
  transform: translate(-50%, -50%);
  cursor: nwse-resize;
}

.handle--ne {
  top: 0;
  right: 0;
  transform: translate(50%, -50%);
  cursor: nesw-resize;
}

.handle--se {
  right: 0;
  bottom: 0;
  transform: translate(50%, 50%);
  cursor: nwse-resize;
}

.handle--sw {
  bottom: 0;
  left: 0;
  transform: translate(-50%, 50%);
  cursor: nesw-resize;
}

.text {
  width: 100%;
  height: 100%;
  overflow: hidden;
  white-space: pre-wrap;
  line-height: 1.5;
  word-break: break-word;
}

.text--editing {
  overflow: auto;
  outline: none;
  cursor: text;
  user-select: text;
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
