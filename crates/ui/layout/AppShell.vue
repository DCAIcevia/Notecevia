<script setup lang="ts">
// 应用骨架：常驻侧栏 + 铺满的内容区。
// 窗口是一块底（12 的缝、8 的内缩），侧栏与内容是两块浮在上面板。
// 这里不放业务，只放导航与槽位。
import { useRoute } from "vue-router";

const route = useRoute();

const items = [
  { name: "home", label: "画板" },
  { name: "library", label: "册与书架" },
  { name: "trash", label: "回收站" },
] as const;

function isActive(name: string): boolean {
  const current = String(route.name);
  if (name === "library") return current === "library" || current === "volume" || current === "shelf";
  return current === name;
}
</script>

<template>
  <main class="shell">
    <aside class="shell__side island">
      <div class="shell__brand">Notecevia</div>
      <nav class="shell__nav">
        <RouterLink
          v-for="item in items"
          :key="item.name"
          :to="{ name: item.name }"
          class="shell__item"
          :class="{ 'shell__item--active': isActive(item.name) }"
        >
          {{ item.label }}
        </RouterLink>
      </nav>
    </aside>

    <section class="shell__main island">
      <slot />
    </section>
  </main>
</template>

<style scoped>
.shell {
  display: flex;
  gap: var(--space-2);
  height: 100%;
  padding: var(--space-2);
  background: var(--color-surface-app);
}

.shell__side {
  display: flex;
  flex: none;
  flex-direction: column;
  gap: var(--space-4);
  width: 204px;
  padding: var(--space-4) var(--space-2);
}

.shell__brand {
  padding: 0 var(--space-2);
  font-size: var(--text-sm);
  font-weight: 600;
  color: var(--color-text-muted);
  letter-spacing: 0.04em;
}

.shell__nav {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.shell__item {
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-md);
  color: var(--color-text);
  font-size: var(--text-base);
  text-decoration: none;
}

.shell__item:hover {
  background: var(--color-surface-hover);
}

.shell__item--active {
  background: var(--color-surface-muted);
  font-weight: 600;
}

.shell__main {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
  overflow: hidden;
}
</style>
