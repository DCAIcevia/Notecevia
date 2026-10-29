// 画板文档：加载、编辑、撤销与按策略自动保存。
//
// 文档是前端的整份快照；撤销也存快照，栈最多 100 步，一次手势算一步。
// 脏标记不逐帧比较：改动就置脏，存盘或撤销之后用快照比对一次定下来。

import { defineStore } from "pinia";
import { computed, ref, shallowRef, triggerRef } from "vue";
import { api } from "../api/client";
import type { Body, BlockKind, DocumentDto, Entry, Geometry, IpcFailure } from "../api/types";

const HISTORY_LIMIT = 100;

export const useDocumentStore = defineStore("document", () => {
  const boardId = ref<number | null>(null);
  const document = shallowRef<DocumentDto | null>(null);
  const loading = ref(false);
  const loadError = ref<IpcFailure | null>(null);
  const dirty = ref(false);
  const saving = ref(false);
  const saveError = ref<IpcFailure | null>(null);
  const undoDepth = ref(0);
  const redoDepth = ref(0);

  let saved = "";
  let undoStack: DocumentDto[] = [];
  let redoStack: DocumentDto[] = [];
  let pending: DocumentDto | null = null;
  let timer: number | undefined;
  let intervalMs = 2000;

  const status = computed<"saved" | "dirty" | "saving" | "failed">(() => {
    if (saving.value) return "saving";
    if (saveError.value) return "failed";
    return dirty.value ? "dirty" : "saved";
  });

  function serialize(value: DocumentDto | null): string {
    return JSON.stringify(value);
  }

  async function open(id: number): Promise<void> {
    loading.value = true;
    loadError.value = null;
    saveError.value = null;
    try {
      const loaded = await api.openBoard(id);
      boardId.value = id;
      document.value = loaded;
      saved = serialize(loaded);
      dirty.value = false;
      undoStack = [];
      redoStack = [];
      undoDepth.value = 0;
      redoDepth.value = 0;
    } catch (error) {
      loadError.value = error as IpcFailure;
    } finally {
      loading.value = false;
    }
  }

  function entries(): Entry[] {
    return document.value?.entries ?? [];
  }

  function entry(id: number): Entry | undefined {
    return document.value?.entries.find((item) => item.id === id);
  }

  function nextEntryId(): number {
    const ids = document.value?.entries.map((item) => item.id) ?? [];
    return ids.length === 0 ? 1 : Math.max(...ids) + 1;
  }

  /** 新建块的默认正文；媒体块要等文件进来，暂不给正文。 */
  function defaultBody(kind: BlockKind, id: number): Body | null {
    switch (kind) {
      case "text":
        return { kind: "text", id, text: "" };
      case "table":
        return { kind: "table", id, rows: 3, cols: 3, cells: Array.from({ length: 9 }, () => "") };
      case "shape":
        return {
          kind: "shape",
          id,
          shape: "rect",
          fill: [233, 240, 252],
          stroke: [52, 102, 201],
          strokeWidth: 2,
        };
      case "highlight":
        return { kind: "highlight", id, color: [255, 224, 102], opacity: 0.35 };
      default:
        return null;
    }
  }

  /** 加一个块，返回它的编号。撤销与置脏由调用方用 beginGesture／endGesture 包住。 */
  function addEntry(kind: BlockKind, geometry: Geometry): number {
    if (!document.value) return 0;
    const id = nextEntryId();
    const created: Entry = { id, kind, geometry, z: 0, locked: false, visible: true };
    const body = defaultBody(kind, id);
    document.value.entries = [...document.value.entries, created];
    if (body) document.value.bodies = [...document.value.bodies, body];
    triggerRef(document);
    return id;
  }

  function removeEntry(id: number): void {
    if (!document.value) return;
    document.value.entries = document.value.entries.filter((item) => item.id !== id);
    document.value.bodies = document.value.bodies.filter((item) => item.id !== id);
    triggerRef(document);
  }

  /** 文本编辑逐次写回正文；撤销由编辑开始与结束的两个手势边界决定。 */
  function setText(id: number, value: string): void {
    const body = document.value?.bodies.find((item) => item.id === id);
    if (!body || body.kind !== "text" || body.text === value) return;
    body.text = value;
    triggerRef(document);
  }

  /** 拖动时逐帧调用：只改坐标，置脏与撤销交给手势结束。 */
  function moveEntry(id: number, x: number, y: number): void {
    const target = entry(id);
    if (!target) return;
    target.geometry.x = x;
    target.geometry.y = y;
    triggerRef(document);
  }

  /** 一次手势开始：记下改动前的整份文档。 */
  function beginGesture(): void {
    pending = document.value ? structuredClone(document.value) : null;
  }

  /** 手势结束：只有真的变了才进撤销栈。 */
  function endGesture(): void {
    const before = pending;
    pending = null;
    if (!before || !document.value) return;
    if (serialize(before) === serialize(document.value)) return;
    undoStack = [...undoStack, before].slice(-HISTORY_LIMIT);
    redoStack = [];
    undoDepth.value = undoStack.length;
    redoDepth.value = 0;
    touch();
  }

  /** 改了内容：置脏并按策略排一次存盘。 */
  function touch(): void {
    dirty.value = true;
    if (timer === undefined) {
      timer = window.setTimeout(() => {
        timer = undefined;
        void saveNow();
      }, intervalMs);
    }
  }

  /** 存盘策略来自应用配置，改配置后下一次排期生效。 */
  function usePolicy(autosaveIntervalMs: number): void {
    intervalMs = Math.max(200, autosaveIntervalMs);
  }

  async function saveNow(): Promise<void> {
    const current = document.value;
    if (!current || boardId.value === null || saving.value) return;
    saving.value = true;
    try {
      await api.saveBoard(boardId.value, current);
      saved = serialize(current);
      saveError.value = null;
    } catch (error) {
      saveError.value = error as IpcFailure;
    } finally {
      saving.value = false;
      dirty.value = serialize(document.value) !== saved;
      // 失败不自动重排：错误常驻到下一次成功，重试由下一次改动或手动触发。
      if (dirty.value && !saveError.value) touch();
    }
  }

  function restore(next: DocumentDto | undefined): void {
    if (!next) return;
    document.value = next;
    dirty.value = serialize(next) !== saved;
    if (dirty.value) touch();
  }

  function undo(): void {
    const previous = undoStack.pop();
    if (!previous || !document.value) return;
    redoStack = [...redoStack, document.value];
    undoDepth.value = undoStack.length;
    redoDepth.value = redoStack.length;
    restore(previous);
  }

  function redo(): void {
    const next = redoStack.pop();
    if (!next || !document.value) return;
    undoStack = [...undoStack, document.value];
    undoDepth.value = undoStack.length;
    redoDepth.value = redoStack.length;
    restore(next);
  }

  return {
    boardId,
    document,
    loading,
    loadError,
    dirty,
    saving,
    saveError,
    status,
    undoDepth,
    redoDepth,
    open,
    entries,
    entry,
    addEntry,
    removeEntry,
    setText,
    moveEntry,
    beginGesture,
    endGesture,
    touch,
    usePolicy,
    saveNow,
    undo,
    redo,
  };
});