<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { ArrowLeft, Highlighter, Maximize, Minus, Plus, Redo2, Square, Table2, Type, Undo2 } from "@lucide/vue";
import BaseButton from "../components/base/BaseButton.vue";
import type { BlockKind } from "../api/types";
import CanvasBlock from "../canvas/CanvasBlock.vue";
import { useViewport, type Box } from "../canvas/useViewport";
import { useConfigStore } from "../stores/config";
import { useDocumentStore } from "../stores/document";

const props = defineProps<{ id: string }>();
const router = useRouter();
const doc = useDocumentStore();
const config = useConfigStore();

const surface = ref<HTMLElement | null>(null);
const selectedId = ref<number | null>(null);
const editingId = ref<number | null>(null);
const viewport = useViewport();
const panning = ref(false);

const boardId = computed(() => Number(props.id));

const blocks = computed(() => {
  const opened = doc.document;
  if (!opened) return [];
  const bodies = new Map(opened.bodies.map((body) => [body.id, body]));
  return opened.entries
    .filter((entry) => entry.visible)
    .slice()
    .sort((left, right) => left.z - right.z || left.id - right.id)
    .map((entry) => ({ entry, body: bodies.get(entry.id) }));
});

const statusLabel = computed(() => {
  switch (doc.status) {
    case "saving":
      return "保存中";
    case "failed":
      return "保存失败";
    case "dirty":
      return "未保存";
    default:
      return "已保存";
  }
});

function contentBox(): Box {
  const entries = doc.entries().filter((entry) => entry.visible);
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

async function boot(): Promise<void> {
  if (!config.snapshot) await config.load();
  doc.usePolicy(config.snapshot?.config.autosave.intervalMs ?? 2000);
  await doc.open(boardId.value);
  await nextTick();
  fitToView();
}

function onWheel(event: WheelEvent): void {
  event.preventDefault();
  const element = surface.value;
  if (!element) return;
  const rect = element.getBoundingClientRect();
  if (event.ctrlKey || event.metaKey) {
    viewport.zoomAt(event.clientX - rect.left, event.clientY - rect.top, event.deltaY < 0 ? 1.1 : 1 / 1.1);
  } else {
    viewport.panBy(-event.deltaX, -event.deltaY);
  }
}

function startEdit(id: number): void {
  const entry = doc.entry(id);
  if (!entry || entry.locked || entry.kind !== "text") return;
  doc.beginGesture();
  selectedId.value = id;
  editingId.value = id;
}

function commitEdit(): void {
  if (editingId.value === null) return;
  editingId.value = null;
  doc.endGesture();
}

/** 新建块落在视口中心。文本建完直接进编辑，一次撤销同时收回创建与输入。 */
function createBlock(kind: BlockKind): void {
  const element = surface.value;
  if (!element || !doc.document) return;
  const size =
    kind === "text"
      ? { width: 240, height: 76 }
      : kind === "table"
        ? { width: 300, height: 160 }
        : { width: 160, height: 160 };
  const centerX = (element.clientWidth / 2 - viewport.x.value) / viewport.scale.value;
  const centerY = (element.clientHeight / 2 - viewport.y.value) / viewport.scale.value;

  doc.beginGesture();
  const id = doc.addEntry(kind, {
    x: centerX - size.width / 2,
    y: centerY - size.height / 2,
    width: size.width,
    height: size.height,
    rotation: 0,
  });
  selectedId.value = id;
  if (kind === "text") {
    editingId.value = id;
  } else {
    doc.endGesture();
  }
}

function removeSelected(): void {
  const id = selectedId.value;
  if (id === null) return;
  doc.beginGesture();
  doc.removeEntry(id);
  doc.endGesture();
  selectedId.value = null;
}

let panOrigin = { x: 0, y: 0 };
let moving: { id: number; startX: number; startY: number; baseX: number; baseY: number; changed: boolean } | null =
  null;

function onPointerDown(event: PointerEvent): void {
  if (event.button !== 0) return;
  const holder = (event.target as HTMLElement | null)?.closest<HTMLElement>("[data-entry]");
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);

  if (holder) {
    const id = Number(holder.dataset.entry);
    if (editingId.value !== null && editingId.value !== id) commitEdit();
    if (editingId.value === id) return;
    const entry = doc.entry(id);
    if (!entry) return;
    selectedId.value = id;
    if (entry.locked) return;
    doc.beginGesture();
    moving = {
      id,
      startX: event.clientX,
      startY: event.clientY,
      baseX: entry.geometry.x,
      baseY: entry.geometry.y,
      changed: false,
    };
    return;
  }

  selectedId.value = null;
  panning.value = true;
  panOrigin = { x: event.clientX, y: event.clientY };
}

function onPointerMove(event: PointerEvent): void {
  if (moving) {
    const dx = (event.clientX - moving.startX) / viewport.scale.value;
    const dy = (event.clientY - moving.startY) / viewport.scale.value;
    if (Math.abs(dx) > 0.5 || Math.abs(dy) > 0.5) moving.changed = true;
    doc.moveEntry(moving.id, moving.baseX + dx, moving.baseY + dy);
    return;
  }
  if (panning.value) {
    viewport.panBy(event.clientX - panOrigin.x, event.clientY - panOrigin.y);
    panOrigin = { x: event.clientX, y: event.clientY };
  }
}

function onDoubleClick(event: MouseEvent): void {
  const holder = (event.target as HTMLElement | null)?.closest<HTMLElement>("[data-entry]");
  if (!holder) return;
  startEdit(Number(holder.dataset.entry));
}

function onPointerUp(): void {
  if (moving) {
    doc.endGesture();
    moving = null;
  }
  panning.value = false;
}

function zoom(factor: number): void {
  const element = surface.value;
  if (!element) return;
  viewport.zoomAt(element.clientWidth / 2, element.clientHeight / 2, factor);
}

function onKeydown(event: KeyboardEvent): void {
  if (event.key === "Escape") {
    if (editingId.value !== null) commitEdit();
    selectedId.value = null;
    return;
  }
  if (editingId.value !== null) return;
  if (event.key === "Delete" || event.key === "Backspace") {
    if (selectedId.value === null) return;
    event.preventDefault();
    removeSelected();
    return;
  }
  if (!(event.ctrlKey || event.metaKey)) return;
  const key = event.key.toLowerCase();
  if (key === "z") {
    event.preventDefault();
    if (event.shiftKey) doc.redo();
    else doc.undo();
  } else if (key === "y") {
    event.preventDefault();
    doc.redo();
  }
}

onMounted(() => {
  void boot();
  window.addEventListener("keydown", onKeydown);
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKeydown);
  if (doc.dirty) void doc.saveNow();
});
</script>

<template>
  <main class="board">
    <header class="board__bar">
      <button class="board__back" type="button" aria-label="返回画板列表" @click="router.push({ name: 'home' })">
        <ArrowLeft :size="16" :stroke-width="1.75" />
      </button>
      <h1 class="board__title">{{ doc.document?.board.title ?? "画板" }}</h1>

      <span v-if="doc.status === 'failed'" class="board__error">{{ doc.saveError?.message }}</span>
      <span class="board__status" :class="{ 'board__status--failed': doc.status === 'failed' }">
        {{ statusLabel }}
      </span>

      <button class="chip" type="button" :disabled="doc.undoDepth === 0" aria-label="撤销" @click="doc.undo()">
        <Undo2 :size="15" :stroke-width="1.75" />
      </button>
      <button class="chip" type="button" :disabled="doc.redoDepth === 0" aria-label="重做" @click="doc.redo()">
        <Redo2 :size="15" :stroke-width="1.75" />
      </button>
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
      @dblclick="onDoubleClick"
    >
      <div class="board__layer" :style="{ transform: viewport.transform.value }">
        <CanvasBlock
          v-for="item in blocks"
          :key="item.entry.id"
          :entry="item.entry"
          :body="item.body"
          :selected="selectedId === item.entry.id"
          :editing="editingId === item.entry.id"
          @text="doc.setText(item.entry.id, $event)"
          @commit="commitEdit"
        />
      </div>

      <div v-if="doc.loading" class="board__overlay"><span class="board__note">正在打开画板</span></div>

      <div v-else-if="doc.loadError" class="board__overlay board__overlay--error">
        <p class="board__error">{{ doc.loadError.message }}</p>
        <div class="board__actions">
          <BaseButton size="sm" @click="boot">重试</BaseButton>
          <BaseButton size="sm" @click="router.push({ name: 'home' })">返回列表</BaseButton>
        </div>
      </div>

      <div v-else-if="blocks.length === 0" class="board__overlay">
        <span class="board__note">这张画板还没有内容</span>
      </div>

      <div v-if="editingId === null" class="board__tools">
        <button class="chip" type="button" aria-label="新建文本" @click="createBlock('text')">
          <Type :size="15" :stroke-width="1.75" />
        </button>
        <button class="chip" type="button" aria-label="新建形状" @click="createBlock('shape')">
          <Square :size="15" :stroke-width="1.75" />
        </button>
        <button class="chip" type="button" aria-label="新建表格" @click="createBlock('table')">
          <Table2 :size="15" :stroke-width="1.75" />
        </button>
        <button class="chip" type="button" aria-label="新建高亮" @click="createBlock('highlight')">
          <Highlighter :size="15" :stroke-width="1.75" />
        </button>
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

.board__error {
  max-width: 40ch;
  overflow: hidden;
  font-size: var(--text-sm);
  color: var(--color-danger);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.board__status {
  flex: none;
  font-size: var(--text-sm);
  color: var(--color-text-muted);
}

.board__status--failed {
  color: var(--color-danger);
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

.board__actions {
  display: flex;
  gap: var(--space-2);
}

.board__tools {
  position: absolute;
  bottom: var(--space-3);
  left: 50%;
  display: flex;
  align-items: center;
  gap: var(--space-1);
  padding: var(--space-1);
  border-radius: var(--radius-md);
  background: var(--color-surface-overlay);
  box-shadow: var(--shadow-overlay);
  transform: translateX(-50%);
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

.chip:hover:not(:disabled) {
  background: var(--color-surface-hover);
  color: var(--color-text);
}

.chip:disabled {
  color: var(--color-text-disabled);
  cursor: default;
}

.board__scale {
  min-width: 40px;
  text-align: center;
  font-size: var(--text-sm);
  color: var(--color-text-muted);
  font-variant-numeric: tabular-nums;
}
</style>
