<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import dayjs from "dayjs";
import BaseButton from "../components/base/BaseButton.vue";
import BaseEmptyState from "../components/base/BaseEmptyState.vue";
import BaseSpinner from "../components/base/BaseSpinner.vue";
import { api } from "../api/client";
import type { TrashItem } from "../api/types";

const router = useRouter();
const items = ref<TrashItem[]>([]);
const loading = ref(true);
const failure = ref<string | null>(null);

const kindLabel: Record<TrashItem["kind"], string> = {
  board: "画板",
  volume: "册",
  shelf: "书架",
};

async function load(): Promise<void> {
  loading.value = true;
  failure.value = null;
  try {
    items.value = await api.listTrash();
  } catch (error) {
    failure.value = (error as { message?: string }).message ?? "读取回收站失败";
  } finally {
    loading.value = false;
  }
}

onMounted(load);

async function restore(item: TrashItem): Promise<void> {
  failure.value = null;
  try {
    await api.restoreTrash(item.folder);
    await load();
  } catch (error) {
    failure.value = (error as { message?: string }).message ?? "还原失败";
  }
}

async function purge(item: TrashItem): Promise<void> {
  failure.value = null;
  try {
    await api.purgeTrashItem(item.folder);
    await load();
  } catch (error) {
    failure.value = (error as { message?: string }).message ?? "彻底删除失败";
  }
}

async function empty(): Promise<void> {
  failure.value = null;
  try {
    await api.emptyTrash();
    await load();
  } catch (error) {
    failure.value = (error as { message?: string }).message ?? "清空失败";
  }
}
</script>

<template>
  <main class="app-frame app-frame--column">
    <header class="trash__bar island">
      <button class="trash__back" type="button" @click="router.push({ name: 'home' })">← 画板</button>
      <h1 class="trash__title">回收站</h1>
      <BaseButton v-if="items.length > 0" size="sm" @click="empty">清空</BaseButton>
    </header>
    <section class="island island--fill">
      <div class="island__inset">

    <p v-if="failure" class="trash__error">{{ failure }}</p>
    <p class="trash__hint">删掉的东西在这里放 30 天，到期自动清掉。</p>

    <div v-if="loading" class="trash__state"><BaseSpinner /></div>

    <BaseEmptyState v-else-if="items.length === 0" title="回收站是空的" hint="删掉的画板、册与书架会先到这里。" />

    <ul v-else class="trash__list">
      <li v-for="item in items" :key="item.folder" class="item list-item">
        <div class="row row--static">
          <span class="row__title">{{ item.title }}</span>
          <span class="row__meta">
            {{ kindLabel[item.kind] }} · {{ dayjs(item.deletedAt * 1000).format("MM-DD HH:mm") }} ·
            {{ item.daysLeft === 0 ? "今天到期" : `还剩 ${item.daysLeft} 天` }}
          </span>
        </div>
        <span class="item__actions">
          <button class="item__action" type="button" @click="restore(item)">还原</button>
          <button class="item__action item__action--danger" type="button" @click="purge(item)">彻底删除</button>
        </span>
      </li>
    </ul>
        </div>
    </section>
  </main>
</template>

<style scoped>
.trash__bar {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-1) var(--space-2) var(--space-1) var(--space-3);
}

.trash__back {
  padding: var(--space-1) var(--space-2);
  border: none;
  border-radius: var(--radius-sm);
  background: none;
  color: var(--color-text-muted);
  font-size: var(--text-sm);
  cursor: pointer;
}

.trash__back:hover {
  background: var(--color-surface-hover);
  color: var(--color-text);
}

.trash__title {
  flex: 1;
  font-size: var(--text-xl);
  font-weight: 600;
}

.trash__error {
  max-width: 760px;
  margin: 0 auto var(--space-2);
  color: var(--color-danger);
}

.trash__hint {
  max-width: 760px;
  margin: 0 auto var(--space-4);
  font-size: var(--text-sm);
  color: var(--color-text-muted);
}

.trash__state {
  display: flex;
  justify-content: center;
  padding: var(--space-12) 0;
}

.trash__list {
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
  border-radius: var(--radius-md);
  background: none;
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

.item__action--danger:hover {
  background: var(--color-danger-soft);
  color: var(--color-danger);
}
</style>