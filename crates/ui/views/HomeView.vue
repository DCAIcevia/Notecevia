<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import AppShell from "../layout/AppShell.vue";
import dayjs from "dayjs";
import BaseButton from "../components/base/BaseButton.vue";
import BaseEmptyState from "../components/base/BaseEmptyState.vue";
import BaseSearchField from "../components/base/BaseSearchField.vue";
import BaseSpinner from "../components/base/BaseSpinner.vue";
import BaseTextField from "../components/base/BaseTextField.vue";
import { api } from "../api/client";
import type { BoardSummary, SearchHit } from "../api/types";
import { useWorkspaceStore } from "../stores/workspace";

const workspace = useWorkspaceStore();
const router = useRouter();

const bootError = ref<string | null>(null);
const creating = ref(false);
const query = ref("");
const hits = ref<SearchHit[]>([]);
const searching = ref(false);
const searchError = ref<string | null>(null);

const renamingId = ref<number | null>(null);
const renamingTitle = ref("");
const actionError = ref<string | null>(null);

const showingHits = computed(() => query.value.trim().length > 0);
const thumbs = ref(new Map<number, string>());
const refreshing = ref(false);

// 缩略图按行加载：列表先画出来，滚到哪读到哪，不把整库读一遍再上屏。
const requested = new Set<number>();
let observer: IntersectionObserver | null = null;

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
    // 一张缩略图读不到不影响列表。
    requested.delete(boardId);
  }
}

function observeRow(element: unknown, boardId: number): void {
  if (!(element instanceof HTMLElement)) return;
  element.dataset.boardId = String(boardId);
  observer?.observe(element);
}

async function refreshIndex(): Promise<void> {
  refreshing.value = true;
  try {
    await api.rebuildIndex();
    thumbs.value = new Map();
    requested.clear();
    await workspace.refreshBoards();
  } catch (error) {
    actionError.value = (error as { message?: string }).message ?? "刷新失败";
  } finally {
    refreshing.value = false;
  }
}

async function boot(): Promise<void> {
  bootError.value = null;
  try {
    if (!workspace.status) await workspace.open();
    await workspace.refreshBoards();
  } catch (error) {
    bootError.value = (error as { message?: string }).message ?? "打开工作区失败";
  }
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
    { rootMargin: "200px" },
  );
  void boot();
});

onBeforeUnmount(() => observer?.disconnect());

function formatTime(seconds: number): string {
  return dayjs(seconds * 1000).format("YYYY-MM-DD HH:mm");
}

async function createBoard(): Promise<void> {
  creating.value = true;
  try {
    const board = await api.createBoard("未命名画板");
    router.push({ name: "board", params: { id: board.id } });
  } finally {
    creating.value = false;
  }
}

function openBoard(id: number): void {
  router.push({ name: "board", params: { id } });
}

async function runSearch(value: string): Promise<void> {
  query.value = value;
  if (!value.trim()) {
    hits.value = [];
    searchError.value = null;
    return;
  }
  searching.value = true;
  try {
    hits.value = await api.searchBoards(value.trim());
    searchError.value = null;
  } catch (error) {
    searchError.value = (error as { message?: string }).message ?? "搜索失败";
  } finally {
    searching.value = false;
  }
}

async function beginRename(board: BoardSummary): Promise<void> {
  actionError.value = null;
  renamingId.value = board.id;
  renamingTitle.value = board.title;
  await nextTick();
}

function cancelRename(): void {
  renamingId.value = null;
  renamingTitle.value = "";
}

async function commitRename(board: BoardSummary): Promise<void> {
  const title = renamingTitle.value.trim();
  if (renamingId.value !== board.id) return;
  cancelRename();
  if (!title || title === board.title) return;
  try {
    await api.renameBoard(board.id, title);
    await workspace.refreshBoards();
  } catch (error) {
    actionError.value = (error as { message?: string }).message ?? "改名失败";
  }
}

async function removeBoard(board: BoardSummary): Promise<void> {
  actionError.value = null;
  try {
    await api.deleteBoard(board.id);
    await workspace.refreshBoards();
  } catch (error) {
    actionError.value = (error as { message?: string }).message ?? "删除失败";
  }
}
</script>

<template>
  <AppShell>
    <header class="home__bar page__bar">
      <h1 class="home__title">画板</h1>
      <BaseSearchField class="home__search" placeholder="搜索画板" @search="runSearch" />
      <BaseButton :disabled="creating" @click="createBoard">新建画板</BaseButton>
      <button class="home__link" type="button" :disabled="refreshing" @click="refreshIndex">刷新</button>
    </header>

    <p v-if="actionError" class="home__error">{{ actionError }}</p>

    <div class="page__body">
        <div v-if="bootError" class="home__state">
      <p class="home__error">{{ bootError }}</p>
      <BaseButton size="sm" @click="boot">重试</BaseButton>
    </div>

    <div v-else-if="workspace.boardsLoading && workspace.boards.length === 0" class="home__state">
      <BaseSpinner />
    </div>

    <div v-else-if="workspace.boardsError" class="home__state">
      <p class="home__error">{{ workspace.boardsError.message }}</p>
      <BaseButton size="sm" @click="workspace.refreshBoards()">重试</BaseButton>
    </div>

    <template v-else-if="showingHits">
      <p v-if="searchError" class="home__error">{{ searchError }}</p>
      <p v-else-if="searching" class="home__hint">正在搜索</p>
      <p v-else-if="hits.length === 0" class="home__hint">没有命中的画板</p>

      <ul class="home__list">
        <li v-for="hit in hits" :key="hit.boardId" class="list-item">
          <button class="row" type="button" @click="openBoard(hit.boardId)">
            <span class="row__title">{{ hit.title }}</span>
            <span class="row__snippet">{{ hit.snippet }}</span>
          </button>
        </li>
      </ul>
    </template>

    <BaseEmptyState
      v-else-if="workspace.boards.length === 0"
      title="还没有画板"
      hint="画板是笔记的容器，先建一张。"
    >
      <template #action>
        <BaseButton variant="primary" @click="createBoard">新建画板</BaseButton>
      </template>
    </BaseEmptyState>

    <ul v-else class="home__list">
      <li v-for="board in workspace.boards" :key="board.id" class="item list-item">
        <BaseTextField
          v-if="renamingId === board.id"
          v-model="renamingTitle"
          class="item__rename"
          placeholder="画板名"
          @keydown.enter.prevent="commitRename(board)"
          @keydown.esc.prevent="cancelRename"
          @blur="commitRename(board)"
        />
        <template v-else>
          <button class="row" type="button" :ref="(el) => observeRow(el, board.id)" @click="openBoard(board.id)">
            <span class="row__main">
              <img v-if="thumbs.get(board.id)" class="row__thumb" :src="thumbs.get(board.id)" alt="" />
              <span v-else class="row__thumb row__thumb--empty" aria-hidden="true" />
              <span class="row__title">{{ board.title }}</span>
            </span>
            <span class="row__meta">{{ board.entryCount }} 块 · {{ formatTime(board.updatedAt) }}</span>
          </button>
          <span class="item__actions">
            <button class="item__action" type="button" @click="beginRename(board)">改名</button>
            <button class="item__action item__action--danger" type="button" @click="removeBoard(board)">
              删除
            </button>
          </span>
        </template>
      </li>
    </ul>
    </div>
  </AppShell>
</template>

<style scoped>
.home__bar {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-2) var(--space-3);
}

.home__title {
  flex: none;
  font-size: var(--text-xl);
  font-weight: 600;
  line-height: var(--leading-tight);
}

.home__search {
  flex: 1;
}

.home__link {
  flex: none;
  padding: var(--space-1) var(--space-2);
  border: none;
  border-radius: var(--radius-sm);
  background: none;
  color: var(--color-text-muted);
  font-size: var(--text-sm);
  cursor: pointer;
}

.home__link:hover {
  background: var(--color-surface-hover);
  color: var(--color-text);
}

.home__state {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-12) 0;
}

.home__error {
  flex: none;
  padding: 0 var(--space-3);
  color: var(--color-danger);
}

.home__hint {
  margin-bottom: var(--space-2);
  font-size: var(--text-sm);
  color: var(--color-text-muted);
}

.home__list {
  display: flex;
  flex-direction: column;
  margin: 0;
  padding: 0;
  list-style: none;
}

.item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.row {
  display: flex;
  flex: 1;
  /* 文字与上方标题对齐：悬停底色向两侧多出一格 */
  margin-inline: calc(-1 * var(--space-3));
  align-items: baseline;
  justify-content: space-between;
  gap: var(--space-4);
  min-width: 0;
  padding: var(--space-3);
  border: none;
  border-radius: var(--radius-md);
  background: none;
  text-align: left;
  cursor: pointer;
  transition: background-color var(--duration-fast) var(--ease-standard);
}

.row:hover {
  background: var(--color-surface-hover);
}

.row__main {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  min-width: 0;
}

.row__thumb {
  flex: none;
  width: 44px;
  height: 30px;
  border-radius: var(--radius-sm);
  object-fit: cover;
  box-shadow: var(--shadow-hairline);
}

.row__thumb--empty {
  background: var(--color-surface-muted);
}

.row__title {
  overflow: hidden;
  font-size: var(--text-base);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.row__meta {
  flex: none;
  font-size: var(--text-sm);
  color: var(--color-text-muted);
  font-variant-numeric: tabular-nums;
}

.row__snippet {
  overflow: hidden;
  font-size: var(--text-sm);
  color: var(--color-text-muted);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.item__actions {
  display: flex;
  flex: none;
  gap: var(--space-1);
  opacity: 0;
  transition: opacity var(--duration-fast) var(--ease-standard);
}

.item:hover .item__actions,
.item:focus-within .item__actions {
  opacity: 1;
}

.item__action {
  padding: var(--space-1) var(--space-2);
  border: none;
  border-radius: var(--radius-sm);
  background: none;
  color: var(--color-text-muted);
  font-size: var(--text-sm);
  cursor: pointer;
}

.item__action:hover {
  background: var(--color-surface-hover);
  color: var(--color-text);
}

.item__action--danger:hover {
  background: var(--color-danger-soft);
  color: var(--color-danger);
}

.item__rename {
  flex: 1;
}

.item__rename :deep(.field) {
  width: 100%;
}

.item__rename :deep(.field__input) {
  width: 100%;
}
</style>