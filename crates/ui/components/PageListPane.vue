<script setup lang="ts">
// 页列表：当前册的页。新建、改名、删除都在这里，
// 选中哪一页由路由决定，列表本身不持有选中态。
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import dayjs from "dayjs";
import BaseInlineField from "./base/BaseInlineField.vue";
import BaseSpinner from "./base/BaseSpinner.vue";
import BaseTextField from "./base/BaseTextField.vue";
import { ArrowUpFromLine, Pencil, Trash } from "../icons";
import { api } from "../api/client";
import type { BoardSummary } from "../api/types";
import { useWorkspaceStore } from "../stores/workspace";

const props = defineProps<{ volumeId: number }>();
const workspace = useWorkspaceStore();
const router = useRouter();
const route = useRoute();

const bootError = ref<string | null>(null);
const creating = ref(false);
const naming = ref(false);
const actionError = ref<string | null>(null);
const renamingId = ref<number | null>(null);
const renamingTitle = ref("");
const refreshInput = ref<{ focus: () => void } | null>(null);
const pages = ref<number[]>([]);

const thumbs = ref(new Map<number, string>());
const requested = new Set<number>();
let observer: IntersectionObserver | null = null;

const selectedId = computed(() => Number(route.params.boardId ?? 0));
const rows = computed(() => {
  const byId = new Map(workspace.boards.map((board) => [board.id, board]));
  return pages.value
    .map((id) => byId.get(id))
    .filter((board): board is BoardSummary => board !== undefined);
});

async function boot(): Promise<void> {
  bootError.value = null;
  try {
    if (!workspace.status) await workspace.open();
    await workspace.refreshBoards();
    await refreshPages();
  } catch (error) {
    bootError.value = (error as { message?: string }).message ?? "打开工作区失败";
  }
}

async function refreshPages(): Promise<void> {
  pages.value = await api.pagesInVolume(props.volumeId);
}

onMounted(() => {
  observer = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue;
        const boardId = Number((entry.target as HTMLElement).dataset.boardId);
        observer?.unobserve(entry.target);
        if (Number.isFinite(boardId)) void loadThumb(boardId);
      }
    },
    { rootMargin: "160px" },
  );
  void boot();
});

onBeforeUnmount(() => observer?.disconnect());

async function loadThumb(boardId: number): Promise<void> {
  if (thumbs.value.has(boardId) || requested.has(boardId)) return;
  requested.add(boardId);
  try {
    const bytes = await api.readThumbnail(boardId);
    if (bytes.byteLength === 0) return;
    const next = new Map(thumbs.value);
    next.set(boardId, URL.createObjectURL(new Blob([bytes], { type: "image/png" })));
    thumbs.value = next;
  } catch {
    requested.delete(boardId);
  }
}

function observeRow(element: unknown, boardId: number): void {
  if (!(element instanceof HTMLElement)) return;
  element.dataset.boardId = String(boardId);
  observer?.observe(element);
}

function formatTime(seconds: number): string {
  return dayjs(seconds * 1000).format("MM-DD HH:mm");
}

function beginCreate(): void {
  actionError.value = null;
  naming.value = true;
}

async function createPage(title: string): Promise<void> {
  naming.value = false;
  creating.value = true;
  actionError.value = null;
  try {
    const created = await api.createPage(title, props.volumeId);
    await workspace.refreshBoards();
    await refreshPages();
    await router.push({
      name: "volume",
      params: { volumeId: props.volumeId, boardId: created.board.id },
    });
  } catch (error) {
    actionError.value = (error as { message?: string }).message ?? "新建页失败";
  } finally {
    creating.value = false;
  }
}

function beginRename(board: BoardSummary, event: Event): void {
  event.preventDefault();
  event.stopPropagation();
  actionError.value = null;
  renamingId.value = board.id;
  renamingTitle.value = board.title;
  void nextTick(() => refreshInput.value?.focus());
}

function cancelRename(): void {
  renamingId.value = null;
  renamingTitle.value = "";
}

async function commitRename(board: BoardSummary): Promise<void> {
  const title = renamingTitle.value.trim();
  const id = renamingId.value;
  if (id === null) return;
  cancelRename();
  if (!title || title === board.title) return;
  try {
    await api.renameBoard(id, title);
    await workspace.refreshBoards();
  } catch (error) {
    actionError.value = (error as { message?: string }).message ?? "改名失败";
  }
}

async function detachBoard(board: BoardSummary, event: Event): Promise<void> {
  event.preventDefault();
  event.stopPropagation();
  actionError.value = null;
  try {
    await api.unassignPage(board.id);
    await refreshPages();
    if (selectedId.value === board.id) await router.push({ name: "home" });
  } catch (error) {
    actionError.value = (error as { message?: string }).message ?? "移出失败";
  }
}

async function removeBoard(board: BoardSummary, event: Event): Promise<void> {
  event.preventDefault();
  event.stopPropagation();
  actionError.value = null;
  try {
    await api.deleteBoard(board.id);
    await workspace.refreshBoards();
    await refreshPages();
    if (selectedId.value === board.id) await router.push({ name: "home" });
  } catch (error) {
    actionError.value = (error as { message?: string }).message ?? "删除失败";
  }
}
</script>

<template>
  <div class="pane">
    <div class="pane__head">
      <BaseInlineField
        v-if="naming"
        class="pane__naming"
        placeholder="页名"
        label="新建页"
        @submit="createPage"
        @cancel="naming = false"
      />
      <template v-else>
        <span class="pane__label">页</span>
        <button class="pane__new" type="button" :disabled="creating" aria-label="新建页" @click="beginCreate">
          ＋
        </button>
      </template>
    </div>

    <div class="pane__scroll">
      <p v-if="actionError" class="pane__error">{{ actionError }}</p>

      <div v-if="bootError" class="pane__state">
        <p class="pane__error">{{ bootError }}</p>
        <button class="pane__retry" type="button" @click="boot">重试</button>
      </div>

      <div v-else-if="workspace.boardsLoading && rows.length === 0" class="pane__state">
        <BaseSpinner />
      </div>

      <template v-else-if="workspace.boardsError">
        <p class="pane__error">{{ workspace.boardsError.message }}</p>
      </template>

      <p v-else-if="rows.length === 0" class="pane__hint">这一册还没有页，点右上角新建。</p>

      <template v-else>
        <BaseTextField
          v-if="renamingId !== null"
          ref="refreshInput"
          v-model="renamingTitle"
          class="pane__rename"
          placeholder="页名"
          @keydown.enter.prevent="commitRename(rows.find((b) => b.id === renamingId)!)"
          @keydown.esc.prevent="cancelRename"
          @blur="commitRename(rows.find((b) => b.id === renamingId)!)"
        />

        <RouterLink
          v-for="board in rows"
          :key="board.id"
          :ref="(el) => observeRow(el, board.id)"
          :to="{ name: 'volume', params: { volumeId: props.volumeId, boardId: board.id } }"
          class="row"
          :class="{ 'row--active': selectedId === board.id }"
        >
          <img v-if="thumbs.get(board.id)" class="row__thumb" :src="thumbs.get(board.id)" alt="" />
          <span v-else class="row__thumb row__thumb--empty" aria-hidden="true" />
          <span class="row__text">
            <span class="row__title">{{ board.title }}</span>
            <span class="row__meta">{{ board.entryCount }} 块 · {{ formatTime(board.updatedAt) }}</span>
          </span>
          <span class="row__actions">
            <button
              class="row__action"
              type="button"
              aria-label="重命名页"
              title="重命名"
              @click="beginRename(board, $event)"
            >
              <Pencil :size="14" :stroke-width="1.75" />
            </button>
            <button
              class="row__action"
              type="button"
              aria-label="移出这一册"
              title="移出"
              @click="detachBoard(board, $event)"
            >
              <ArrowUpFromLine :size="14" :stroke-width="1.75" />
            </button>
            <button
              class="row__action row__action--danger"
              type="button"
              aria-label="删除页"
              title="删除"
              @click="removeBoard(board, $event)"
            >
              <Trash :size="14" :stroke-width="1.75" />
            </button>
          </span>
        </RouterLink>
      </template>
    </div>
  </div>
</template>

<style scoped>
.pane {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}

.pane__head {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--space-1);
  padding: var(--space-2);
  border-bottom: 1px solid var(--color-line);
}

.pane__label {
  flex: 1;
  padding-left: var(--space-2);
  font-size: var(--text-sm);
  color: var(--color-text-muted);
}

.pane__naming {
  flex: 1;
}

.pane__new {
  flex: none;
  width: 26px;
  height: var(--control-height-md);
  border: 1px solid var(--color-line-strong);
  border-radius: var(--radius-md);
  background: var(--color-surface-content);
  color: var(--color-text-muted);
  cursor: pointer;
}

.pane__new:hover:not(:disabled) {
  background: var(--color-surface-hover);
  color: var(--color-text);
}

.pane__scroll {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: var(--space-1) var(--space-1) var(--space-3);
}

.pane__error {
  padding: 0 var(--space-2);
  color: var(--color-danger);
  font-size: var(--text-sm);
}

.pane__hint {
  padding: var(--space-2);
  color: var(--color-text-muted);
  font-size: var(--text-sm);
}

.pane__state {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-8) 0;
}

.pane__retry {
  padding: var(--space-1) var(--space-2);
  border: none;
  border-radius: var(--radius-sm);
  background: none;
  color: var(--color-text-muted);
  cursor: pointer;
}

.pane__rename {
  width: calc(100% - var(--space-2));
  margin: var(--space-1) var(--space-1);
}

.pane__rename :deep(.field),
.pane__rename :deep(.field__input) {
  width: 100%;
}

.row {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2);
  border-radius: var(--radius-md);
  color: inherit;
  text-decoration: none;
}

.row:hover {
  background: var(--color-surface-hover);
}

.row--active {
  background: var(--color-surface-muted);
}

.row__thumb {
  flex: none;
  width: 38px;
  height: 26px;
  border-radius: var(--radius-sm);
  object-fit: cover;
  box-shadow: var(--shadow-hairline);
}

.row__thumb--empty {
  background: var(--color-surface-muted);
}

.row__text {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
}

.row__title {
  overflow: hidden;
  font-size: var(--text-base);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.row__meta {
  overflow: hidden;
  font-size: var(--text-xs);
  color: var(--color-text-muted);
  text-overflow: ellipsis;
  white-space: nowrap;
  font-variant-numeric: tabular-nums;
}

.row__actions {
  display: flex;
  flex: none;
  gap: 2px;
  opacity: 0;
}

.row:hover .row__actions,
.row:focus-within .row__actions {
  opacity: 1;
}

.row__action {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border: none;
  border-radius: var(--radius-sm);
  background: none;
  color: var(--color-text-muted);
  cursor: pointer;
}

.row__action:hover {
  background: var(--color-surface-content);
  color: var(--color-text);
}

.row__action--danger:hover {
  background: var(--color-danger-soft);
  color: var(--color-danger);
}
</style>
