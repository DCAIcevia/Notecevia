<script setup lang="ts">
import { computed, nextTick, onMounted, ref, shallowRef } from "vue";
import { useRouter } from "vue-router";
import { ArrowLeft, Maximize, Minus, Plus } from "@lucide/vue";
import { api } from "../api/client";
import type { Body, DocumentDto, IpcFailure } from "../api/types";
import CanvasBlock from "../canvas/CanvasBlock.vue";
import { useViewport, type Box } from "../canvas/useViewport";

const props = defineProps<{ id: string }>();
const router = useRouter();

const loaded = shallowRef<DocumentDto | null>(null);
const loading = ref(true);
const error = ref<IpcFailure | null>(null);
const surface = ref<HTMLElement | null>(null);
const panning = ref(false);

const viewport = useViewport();
const boardId = computed(() => Number(props.id));

const blocks = computed(() => {
  const opened = loaded.value;
  if (!opened) return [];
  const bodies = new Map<number, Body>();
  for (const body of opened.bodies) bodies.set(body.id, body);
  return opened.entries
    .filter((entry) => entry.visible)
    .slice()
    .sort((left, right) => left.z - right.z || left.id - right.id)
    .map((entry) => ({ entry, body: bodies.get(entry.id) }));
});

function contentBox(): Box {
  const entries = loaded.value?.entries.filter((entry) => entry.visible) ?? [];
  if (entries.length === 0) return { minX: 0, minY: 0, maxX: 0, maxY: 0 };
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  for (const { geometry } of entries) {
    minX = Math.min(minX, geometry.x);
    minY = Math.min(minY, geometry.y);
    maxX = Math.max(maxX, geometry.x + geometry.width);
    maxY = Math.max(maxY, geometry.y + geometry.height);
  }
  return { minX, minY, maxX, maxY };
}

function fitToView(): void {
  const element = surface.value;
  if (!element) return;
  viewport.fitContent(contentBox(), element.clientWidth, element.clientHeight);
}

async function load(): Promise<void> {
  loading.value = true;
  error.value = null;
  try {
    loaded.value = await api.openBoard(boardId.value);
    await nextTick();
    fitToView();
  } catch (failure) {
    error.value = failure as IpcFailure;
  } finally {
    loading.value = false;
  }
}

function onWheel(event: WheelEvent): void {
  event.preventDefault();
  const element = surface.value;
  if (!element) return;
  const rect = element.getBoundingClientRect();
  if (event.ctrlKey || event.metaKey) {
    const factor = event.deltaY < 0 ? 1.1 : 1 / 1.1;
    viewport.zoomAt(event.clientX - rect.left, event.clientY - rect.top, factor);
  } else {
    viewport.panBy(-event.deltaX, -event.deltaY);
  }
}

let origin = { x: 0, y: 0 };

function onPointerDown(event: PointerEvent): void {
  if (event.button !== 0) return;
  panning.value = true;
  origin = { x: event.clientX, y: event.clientY };
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
}

function onPointerMove(event: PointerEvent): void {
  if (!panning.value) return;
  viewport.panBy(event.clientX - origin.x, event.clientY - origin.y);
  origin = { x: event.clientX, y: event.clientY };
}

function onPointerUp(): void {
  panning.value = false;
}

function zoom(factor: number): void {
  const element = surface.value;
  if (!element) return;
  viewport.zoomAt(element.clientWidth / 2, element.clientHeight / 2, factor);
}

onMounted(load);
</script>

<template>
  <main class="board">
    <header class="board__bar">
      <button class="board__back" type="button" aria-label="返回画板列表" @click="router.push({ name: 'home' })">
        <ArrowLeft :size="16" :stroke-width="1.75" />
      </button>
      <h1 class="board__title">{{ loaded?.board.title ?? "画板" }}</h1>
      <span class="board__hint">只读预览</span>
    </header>

    <div
      ref="surface"
      class="board__surface"
      :class="{ 'board__surface--panning': panning }"
      @wheel="onWheel"
      @pointerdown="onPointerDown"
      @pointermove="onPointerMove"
      @pointerup="onPointerUp"
      @pointercancel="onPointerUp"
    >
      <div class="board__layer" :style="{ transform: viewport.transform.value }">
        <CanvasBlock v-for="item in blocks" :key="item.entry.id" :entry="item.entry" :body="item.body" />
      </div>

      <div v-if="loading" class="board__overlay"><span class="board__note">正在打开画板</span></div>

      <div v-else-if="error" class="board__overlay board__overlay--error">
        <p class="board__error">{{ error.message }}</p>
        <div class="board__actions">
          <button class="chip" type="button" @click="load">重试</button>
          <button class="chip" type="button" @click="router.push({ name: 'home' })">返回列表</button>
        </div>
      </div>

      <div v-else-if="blocks.length === 0" class="board__overlay">
        <span class="board__note">这张画板还没有内容</span>
      </div>

      <div class="board__zoom">
        <button class="chip" type="button" aria-label="缩小" @click="zoom(1 / 1.2)">
          <Minus :size="14" :stroke-width="1.75" />
        </button>
        <span class="board__scale">{{ Math.round(viewport.scale.value * 100) }}%</span>
        <button class="chip" type="button" aria-label="放大" @click="zoom(1.2)">
          <Plus :size="14" :stroke-width="1.75" />
        </button>
        <button class="chip" type="button" aria-label="适应窗口" @click="fitToView">
          <Maximize :size="14" :stroke-width="1.75" />
        </button>
      </div>
    </div>
  </main>
</template>

<style scoped>
.board {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--color-surface-content);
}

.board__bar {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  border-bottom: 1px solid var(--color-line);
  background: var(--color-surface-content);
}

.board__back {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  border: none;
  border-radius: var(--radius-sm);
  background: none;
  color: var(--color-text-muted);
  cursor: pointer;
}

.board__back:hover {
  background: var(--color-surface-hover);
  color: var(--color-text);
}

.board__title {
  flex: 1;
  min-width: 0;
  font-size: var(--text-base);
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.board__hint {
  flex: none;
  font-size: var(--text-sm);
  color: var(--color-text-muted);
}

.board__surface {
  position: relative;
  flex: 1;
  overflow: hidden;
  cursor: grab;
  touch-action: none;
}

.board__surface--panning {
  cursor: grabbing;
}

.board__layer {
  position: absolute;
  top: 0;
  left: 0;
  transform-origin: 0 0;
  will-change: transform;
}

.board__overlay {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-3);
  pointer-events: none;
}

.board__overlay--error {
  pointer-events: auto;
}

.board__note {
  font-size: var(--text-sm);
  color: var(--color-text-muted);
}

.board__error {
  color: var(--color-danger);
  max-width: 32em;
}

.board__actions {
  display: flex;
  gap: var(--space-2);
}

.board__zoom {
  position: absolute;
  right: var(--space-3);
  bottom: var(--space-3);
  display: flex;
  align-items: center;
  gap: var(--space-1);
  padding: var(--space-1);
  border-radius: var(--radius-md);
  background: var(--color-surface-overlay);
  box-shadow: var(--shadow-overlay);
}

.chip {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  height: 24px;
  min-width: 24px;
  padding: 0 var(--space-1);
  border: none;
  border-radius: var(--radius-sm);
  background: none;
  color: var(--color-text-muted);
  font-size: var(--text-sm);
  cursor: pointer;
}

.chip:hover {
  background: var(--color-surface-hover);
  color: var(--color-text);
}

.board__scale {
  min-width: 40px;
  text-align: center;
  font-size: var(--text-sm);
  color: var(--color-text-muted);
  font-variant-numeric: tabular-nums;
}
</style>
