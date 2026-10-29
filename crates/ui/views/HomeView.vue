<script setup lang="ts">
import AppShell from "../layout/AppShell.vue";
import BoardListPane from "../components/BoardListPane.vue";
</script>

<template>
  <AppShell>
    <template #list>
      <BoardListPane />
    </template>

    <div class="blank">
      <img class="blank__lockup" src="/lockup.svg" alt="Notecevia" />
      <p class="blank__title">还没有打开画板</p>
      <p class="blank__hint">从左边挑一张，或者新建一张。</p>
    </div>
  </AppShell>
</template>

<style scoped>
.blank {
  display: flex;
  flex: 1;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-1);
}

.blank__lockup {
  height: 48px;
  margin-bottom: var(--space-3);
  opacity: 0.9;
}

.blank__title {
  font-size: var(--text-lg);
  color: var(--color-text-muted);
}

.blank__hint {
  font-size: var(--text-sm);
  color: var(--color-text-muted);
}
</style>