<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import AppShell from "../layout/AppShell.vue";
import BaseSpinner from "../components/base/BaseSpinner.vue";
import { api } from "../api/client";
import type { ShelfDto, VolumeSummary } from "../api/types";

const props = defineProps<{ id: string }>();
const router = useRouter();

const shelf = ref<ShelfDto | null>(null);
const volumes = ref<VolumeSummary[]>([]);
const loading = ref(true);
const failure = ref<string | null>(null);

const shelfId = computed(() => Number(props.id));
const titles = computed(() => new Map(volumes.value.map((volume) => [volume.id, volume.title])));

async function load(): Promise<void> {
  loading.value = true;
  failure.value = null;
  try {
    shelf.value = await api.loadShelf(shelfId.value);
    volumes.value = await api.listVolumes();
  } catch (error) {
    failure.value = (error as { message?: string }).message ?? "读不到这层书架";
  } finally {
    loading.value = false;
  }
}

onMounted(load);

async function save(volumesInShelf: number[]): Promise<void> {
  if (!shelf.value) return;
  failure.value = null;
  try {
    await api.saveShelf(shelfId.value, { title: shelf.value.title, volumes: volumesInShelf });
    await load();
  } catch (error) {
    failure.value = (error as { message?: string }).message ?? "保存失败";
  }
}

function add(volumeId: number): void {
  if (!shelf.value || shelf.value.volumes.includes(volumeId)) return;
  void save([...shelf.value.volumes, volumeId]);
}

function remove(volumeId: number): void {
  if (!shelf.value) return;
  void save(shelf.value.volumes.filter((id) => id !== volumeId));
}
</script>

<template>
  <AppShell>
    <header class="shelf__bar page__bar">
      <button class="shelf__back" type="button" @click="router.push({ name: 'library' })">← 册与书架</button>
      <h1 class="shelf__title">{{ shelf?.title ?? "书架" }}</h1>
    </header>
    <div class="page__body">

    <p v-if="failure" class="shelf__error">{{ failure }}</p>

    <div v-if="loading" class="shelf__state"><BaseSpinner /></div>

    <template v-else-if="shelf">
      <section class="shelf__section">
        <h2 class="shelf__heading">架上的册</h2>
        <p v-if="shelf.volumes.length === 0" class="shelf__hint">这一层还空着，从下面加册。</p>
        <ul class="shelf__list">
          <li v-for="volumeId in shelf.volumes" :key="volumeId" class="item list-item">
            <button class="row" type="button" @click="router.push({ name: 'volume', params: { id: volumeId } })">
              <span class="row__title">{{ titles.get(volumeId) ?? `册 ${volumeId}` }}</span>
            </button>
            <span class="item__actions">
              <button class="item__action" type="button" @click="remove(volumeId)">移出</button>
            </span>
          </li>
        </ul>
      </section>

      <section class="shelf__section">
        <h2 class="shelf__heading">还没上架</h2>
        <ul class="shelf__list">
          <li
            v-for="volume in volumes.filter((item) => !shelf?.volumes.includes(item.id))"
            :key="volume.id"
            class="item"
          >
            <div class="row row--static">
              <span class="row__title">{{ volume.title }}</span>
              <span class="row__meta">{{ volume.pageCount }} 页</span>
            </div>
            <span class="item__actions">
              <button class="item__action" type="button" @click="add(volume.id)">上架</button>
            </span>
          </li>
        </ul>
      </section>
    </template>
      </div>
  </AppShell>
</template>

<style scoped>
.shelf__bar {
  gap: var(--space-3);
}

.shelf__back {
  padding: var(--space-1) var(--space-2);
  border: none;
  border-radius: var(--radius-sm);
  background: none;
  color: var(--color-text-muted);
  font-size: var(--text-sm);
  cursor: pointer;
}

.shelf__back:hover {
  background: var(--color-surface-hover);
  color: var(--color-text);
}

.shelf__title {
  font-size: var(--text-xl);
  font-weight: 600;
}

.shelf__error {
  max-width: 760px;
  margin: 0 auto var(--space-2);
  color: var(--color-danger);
}

.shelf__state {
  display: flex;
  justify-content: center;
  padding: var(--space-12) 0;
}

.shelf__section {
  max-width: 760px;
  margin: 0 auto var(--space-8);
}

.shelf__heading {
  margin-bottom: var(--space-3);
  font-size: var(--text-sm);
  font-weight: 600;
  color: var(--color-text-muted);
}

.shelf__hint {
  margin-bottom: var(--space-2);
  font-size: var(--text-sm);
  color: var(--color-text-muted);
}

.shelf__list {
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
}

.row:hover {
  background: var(--color-surface-hover);
}

.row--static {
  cursor: default;
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

.item__action:hover {
  background: var(--color-surface-hover);
  color: var(--color-text);
}
</style>