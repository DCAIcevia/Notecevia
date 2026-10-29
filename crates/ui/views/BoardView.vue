<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import {
  Download,
  FileCode2,
  Highlighter,
  ImagePlus,
  Maximize,
  Minus,
  Plus,
  Redo2,
  Square,
  Table2,
  Type,
  Undo2,
} from "../icons";
import { api } from "../api/client";
import AppShell from "../layout/AppShell.vue";
import BaseButton from "../components/base/BaseButton.vue";
import BoardListPane from "../components/BoardListPane.vue";
import type { BlockKind } from "../api/types";
import CanvasBlock from "../canvas/CanvasBlock.vue";
import { renderThumbnail } from "../canvas/thumbnail";
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
const picker = ref<HTMLInputElement | null>(null);
const importing = ref(false);
const importError = ref<string | null>(null);
const exportedPath = ref<string | null>(null);
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
  // 配置与打开画板互不依赖：并行跑，打开不等配置。
  const configReady = config.snapshot ? Promise.resolve() : config.load();
  await doc.open(boardId.value);
  await nextTick();
  fitToView();
  await configReady;
  doc.usePolicy(config.snapshot?.config.autosave.intervalMs ?? 2000);
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

function pickMedia(): void {
  picker.value?.click();
}

/** 媒体先写进媒体池换成哈希，再把块挂进文档；一次插入算一步撤销。 */
async function onFiles(event: Event): Promise<void> {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  input.value = "";
  if (!file || !doc.document) return;
  const kind = file.type.startsWith("image/")
    ? "image"
    : file.type.startsWith("video/")
      ? "video"
      : file.type.startsWith("audio/")
        ? "audio"
        : null;
  if (!kind) {
    importError.value = "只认图片、视频与音频";
    return;
  }

  importing.value = true;
  importError.value = null;
  try {
    const bytes = new Uint8Array(await file.arrayBuffer());
    const hash = await api.writeMedia(file.name, Array.from(bytes));
    const element = surface.value;
    const width = kind === "image" ? 320 : 360;
    const height = kind === "image" ? 240 : 200;
    const centerX = element ? (element.clientWidth / 2 - viewport.x.value) / viewport.scale.value : 0;
    const centerY = element ? (element.clientHeight / 2 - viewport.y.value) / viewport.scale.value : 0;

    doc.beginGesture();
    const id = doc.addMediaEntry(
      kind,
      { x: centerX - width / 2, y: centerY - height / 2, width, height, rotation: 0 },
      { hash, name: file.name, bytes: bytes.byteLength },
    );
    doc.endGesture();
    selectedId.value = id;
  } catch (error) {
    importError.value = (error as { message?: string }).message ?? "导入失败";
  } finally {
    importing.value = false;
  }
}

async function exportAs(kind: "container" | "html"): Promise<void> {
  importError.value = null;
  exportedPath.value = null;
  try {
    exportedPath.value =
      kind === "container"
        ? await api.exportContainer(boardId.value)
        : await api.exportHtml(boardId.value);
  } catch (error) {
    importError.value = (error as { message?: string }).message ?? "导出失败";
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
let resizing: {
  id: number;
  corner: string;
  startX: number;
  startY: number;
  base: { x: number; y: number; width: number; height: number };
} | null = null;

let rotating: {
  id: number;
  centerX: number;
  centerY: number;
  baseRotation: number;
  startAngle: number;
} | null = null;

/** 指针在世界坐标里的位置。 */
function worldPoint(event: PointerEvent): { x: number; y: number } {
  const element = surface.value;
  if (!element) return { x: 0, y: 0 };
  const rect = element.getBoundingClientRect();
  return {
    x: (event.clientX - rect.left - viewport.x.value) / viewport.scale.value,
    y: (event.clientY - rect.top - viewport.y.value) / viewport.scale.value,
  };
}

function onHandleDown(event: PointerEvent, handle: HTMLElement): void {
  const holder = handle.closest<HTMLElement>("[data-entry]");
  if (!holder) return;
  const id = Number(holder.dataset.entry);
  const entry = doc.entry(id);
  if (!entry || entry.locked) return;
  event.stopPropagation();
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);

  if (handle.dataset.handle === "rotate") {
    doc.beginGesture();
    const point = worldPoint(event);
    const centerX = entry.geometry.x + entry.geometry.width / 2;
    const centerY = entry.geometry.y + entry.geometry.height / 2;
    rotating = {
      id,
      centerX,
      centerY,
      baseRotation: entry.geometry.rotation,
      startAngle: Math.atan2(point.y - centerY, point.x - centerX),
    };
    return;
  }

  doc.beginGesture();
  resizing = {
    id,
    corner: handle.dataset.handle ?? "se",
    startX: event.clientX,
    startY: event.clientY,
    base: { ...entry.geometry },
  };
}

function applyRotate(event: PointerEvent): void {
  if (!rotating) return;
  const point = worldPoint(event);
  const angle = Math.atan2(point.y - rotating.centerY, point.x - rotating.centerX);
  doc.setGeometry(rotating.id, { rotation: rotating.baseRotation + (angle - rotating.startAngle) });
}

function applyResize(event: PointerEvent): void {
  if (!resizing) return;
  const dx = (event.clientX - resizing.startX) / viewport.scale.value;
  const dy = (event.clientY - resizing.startY) / viewport.scale.value;
  const { base, corner } = resizing;
  const MIN = 20;
  let { x, y } = base;
  let width = base.width;
  let height = base.height;

  if (corner.includes("w")) {
    x = base.x + dx;
    width = base.width - dx;
  } else if (corner.includes("e")) {
    width = base.width + dx;
  }
  if (corner.includes("n")) {
    y = base.y + dy;
    height = base.height - dy;
  } else if (corner.includes("s")) {
    height = base.height + dy;
  }

  if (width < MIN) {
    width = MIN;
    if (corner.includes("w")) x = base.x + base.width - MIN;
  }
  if (height < MIN) {
    height = MIN;
    if (corner.includes("n")) y = base.y + base.height - MIN;
  }

  doc.setGeometry(resizing.id, { x, y, width, height });
}
let moving: { id: number; startX: number; startY: number; baseX: number; baseY: number; changed: boolean } | null =
  null;

function onPointerDown(event: PointerEvent): void {
  if (event.button !== 0) return;
  const target = event.target as HTMLElement | null;
  const handle = target?.closest<HTMLElement>("[data-handle]");
  if (handle) {
    onHandleDown(event, handle);
    return;
  }
  const holder = target?.closest<HTMLElement>("[data-entry]");
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
  if (rotating) {
    applyRotate(event);
    return;
  }
  if (resizing) {
    applyResize(event);
    return;
  }
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
  if (rotating) {
    doc.endGesture();
    rotating = null;
  }
  if (resizing) {
    doc.endGesture();
    resizing = null;
  }
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

let lastThumbnail = 0;

// 缩略图是缓存：存盘成功后再画，按配置的间隔节流；画不出来不影响保存。
watch(
  () => doc.status,
  async (status) => {
    if (status !== "saved" || !doc.document) return;
    const interval = config.snapshot?.config.autosave.thumbnailIntervalMs ?? 5000;
    if (Date.now() - lastThumbnail < interval) return;
    lastThumbnail = Date.now();
    try {
      const png = await renderThumbnail(doc.document);
      if (png) await api.saveThumbnail(boardId.value, png);
    } catch {
      // 缩略图失败只影响列表里的预览。
    }
  },
);

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
  <AppShell>
    <template #list>
      <BoardListPane />
    </template>

    <div class="board">
    <header class="board__bar island">
      <h1 class="board__title">{{ doc.document?.board.title ?? "画板" }}</h1>

      <span v-if="exportedPath" class="board__status board__status--ok">已导出 {{ exportedPath }}</span>
      <span v-if="importError" class="board__error">{{ importError }}</span>
      <span v-else-if="doc.status === 'failed'" class="board__error">{{ doc.saveError?.message }}</span>
      <span class="board__status" :class="{ 'board__status--failed': doc.status === 'failed' }">
        {{ statusLabel }}
      </span>

      <button class="chip" type="button" :disabled="doc.undoDepth === 0" aria-label="撤销" @click="doc.undo()">
        <Undo2 :size="15" :stroke-width="1.75" />
      </button>
      <button class="chip" type="button" :disabled="doc.redoDepth === 0" aria-label="重做" @click="doc.redo()">
        <Redo2 :size="15" :stroke-width="1.75" />
      </button>
      <button class="chip" type="button" aria-label="导出容器" @click="exportAs('container')">
        <Download :size="15" :stroke-width="1.75" />
      </button>
      <button class="chip" type="button" aria-label="导出 HTML" @click="exportAs('html')">
        <FileCode2 :size="15" :stroke-width="1.75" />
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
          :zoom="viewport.scale.value"
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
        <button class="chip" type="button" :disabled="importing" aria-label="插入媒体" @click="pickMedia">
          <ImagePlus :size="15" :stroke-width="1.75" />
        </button>
        <input
          ref="picker"
          class="board__picker"
          type="file"
          accept="image/*,video/*,audio/*"
          @change="onFiles"
        />
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
    </div>
  </AppShell>
</template>

<style scoped>
.board {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
}

.board__bar {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-1) var(--space-2) var(--space-1) var(--space-3);
  border-bottom: 1px solid var(--color-line);
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

.board__status--ok {
  max-width: 32ch;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 画布是一块铺满的岛：自己承担白底、圆角与最轻的阴影 */
.board__surface {
  position: relative;
  flex: 1;
  min-height: 0;
  overflow: hidden;
  border-radius: var(--radius-xl);
  background: var(--color-surface-content);
  box-shadow: var(--shadow-hairline);
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

.board__picker {
  display: none;
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