<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import AppShell from "../layout/AppShell.vue";
import BaseButton from "../components/base/BaseButton.vue";
import BaseEmptyState from "../components/base/BaseEmptyState.vue";
import BaseSpinner from "../components/base/BaseSpinner.vue";
import { api } from "../api/client";
import type { BoardSummary, VolumeDto } from "../api/types";
import { useWorkspaceStore } from "../stores/workspace";

const props = defineProps<{ id: string }>();
const router = useRouter();
const workspace = useWorkspaceStore();

const volume = ref<VolumeDto | null>(null);
const loading = ref(true);
const failure = ref<string | null>(null);
const candidate = ref<number | null>(null);

const volumeId = computed(() => Number(props.id));
const titles = computed(() => new Map(workspace.boards.map((board: BoardSummary) => [board.id, board.title])));

async function load(): Promise<void> {
  loading.value = true;
  failure.value = null;
  try {
    volume.value = await api.loadVolume(volumeId.value);
    if (workspace.boards.length === 0) await workspace.refreshBoards();
  } catch (error) {
    failure.value = (error as { message?: string }).message ?? "读不到这本册";
  } finally {
    loading.value = false;
  }
}

onMounted(load);

async function bind(): Promise<void> {
  if (candidate.value === null || !volume.value) return;
  failure.value = null;
  try {
    await api.assignPage(volumeId.value, candidate.value, volume.value.pages.length);
    await load();
  } catch (error) {
    failure.value = (error as { message?: string }).message ?? "装订失败";
  }
}

async function unbind(page: number): Promise<void> {
  failure.value = null;
  try {
    await api.unassignPage(page);
    await load();
  } catch (error) {
    failure.value = (error as { message?: string }).message ?? "移出失败";
  }
}

async function move(from: number, to: number): Promise<void> {
  if (!volume.value || to < 0 || to >= volume.value.pages.length) return;
  failure.value = null;
  try {
    await api.movePageInVolume(volumeId.value, from, to);
    await load();
  } catch (error) {
    failure.value = (error as { message?: string }).message ?? "调整顺序失败";
  }
}
</script>

<template>
  <AppShell>
    <header class="volume__bar page__bar">
      <button class="volume__back" type="button" @click="router.push({ name: 'library' })">← 册与书架</button>
      <h1 class="volume__title">{{ volume?.title ?? "册" }}</h1>
    </header>
    <div class="page__body">

    <p v-if="failure" class="volume__error">{{ failure }}</p>

    <div v-if="loading" class="volume__state"><BaseSpinner /></div>

    <template v-else-if="volume">
      <div class="volume__bind">
        <select v-model="candidate" class="volume__select" aria-label="选择要装订的画板">
          <option :value="null">选择一张画板</option>
          <option
            v-for="board in workspace.boards"
            :key="board.id"
            :value="board.id"
            :disabled="volume.pages.includes(board.id)"
          >
            {{ board.title }}
          </option>
        </select>
        <BaseButton size="sm" :disabled="candidate === null" @click="bind">装订进本册</BaseButton>
      </div>

      <BaseEmptyState
        v-if="volume.pages.length === 0"
        title="这本册还是空的"
        hint="从上面选一张画板装订进来。"
      />

      <ol v-else class="volume__list">
        <li v-for="(page, index) in volume.pages" :key="page" class="item">
          <span class="item__index">{{ index + 1 }}</span>
          <button class="row" type="button" @click="router.push({ name: 'board', params: { id: page } })">
            <span class="row__title">{{ titles.get(page) ?? `画板 ${page}` }}</span>
          </button>
          <span class="item__actions">
            <button class="item__action" type="button" :disabled="index === 0" @click="move(index, index - 1)">
              上移
            </button>
            <button
              class="item__action"
              type="button"
              :disabled="index === volume.pages.length - 1"
              @click="move(index, index + 1)"
            >
              下移
            </button>
            <button class="item__action item__action--danger" type="button" @click="unbind(page)">移出</button>
          </span>
        </li>
      </ol>
    </template>
      </div>
  </AppShell>
</template>

<style scoped>
.volume__bar {
  gap: var(--space-3);
}

.volume__back {
  padding: var(--space-1) var(--space-2);
  border: none;
  border-radius: var(--radius-sm);
  background: none;
  color: var(--color-text-muted);
  font-size: var(--text-sm);
  cursor: pointer;
}

.volume__back:hover {
  background: var(--color-surface-hover);
  color: var(--color-text);
}

.volume__title {
  font-size: var(--text-xl);
  font-weight: 600;
}

.volume__error {
  max-width: 760px;
  margin: 0 auto var(--space-2);
  color: var(--color-danger);
}

.volume__state {
  display: flex;
  justify-content: center;
  padding: var(--space-12) 0;
}

.volume__bind {
  display: flex;
  gap: var(--space-2);
  max-width: 760px;
  margin: 0 auto var(--space-4);
}

.volume__select {
  flex: 1;
  height: var(--control-height-md);
  padding: 0 var(--space-2);
  border: 1px solid var(--color-line-strong);
  border-radius: var(--radius-md);
  background: var(--color-surface-content);
  box-shadow: var(--shadow-inset-control);
}

.volume__list {
  display: flex;
  flex-direction: column;
  gap: 2px;
  max-width: 760px;
  margin: 0 auto;
  padding: 0;
  list-style: none;
}

.item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.item__index {
  flex: none;
  width: 2ch;
  text-align: right;
  font-size: var(--text-sm);
  color: var(--color-text-muted);
  font-variant-numeric: tabular-nums;
}

.row {
  display: flex;
  flex: 1;
  /* 文字与上方标题对齐：悬停底色向两侧多出一格 */
  margin-inline: calc(-1 * var(--space-3));
  align-items: center;
  min-width: 0;
  padding: var(--space-3);
  border: none;
  border-radius: var(--radius-md);
  background: none;
  text-align: left;
  cursor: pointer;
}

.row:hover {
  background: var(--color-surface-hover);
}

.row__title {
  overflow: hidden;
  font-size: var(--text-base);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.item__actions {
  display: flex;
  flex: none;
  gap: var(--space-1);
  opacity: 0;
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

.item__action:disabled {
  color: var(--color-text-disabled);
  cursor: default;
}

.item__action--danger:hover:not(:disabled) {
  background: var(--color-danger-soft);
  color: var(--color-danger);
}
</style>