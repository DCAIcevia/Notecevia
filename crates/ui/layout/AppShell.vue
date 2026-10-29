<script setup lang="ts">
// 应用骨架：导航栏 + 可选的次级列表栏 + 主内容。
// 导航栏常驻，切区域只换列表栏与主内容；画板与画布共用同一套列表面板。
import { useRoute } from "vue-router";
import { BookOpen, Grid, Trash } from "../icons";

const route = useRoute();

const items = [
  { name: "home", label: "画板", icon: Grid },
  { name: "library", label: "册与书架", icon: BookOpen },
  { name: "trash", label: "回收站", icon: Trash },
] as const;

function isActive(name: string): boolean {
  const current = String(route.name);
  if (name === "library") return current === "library" || current === "volume" || current === "shelf";
  return current === name;
}
</script>

<template>
  <main class="shell">
    <nav class="shell__rail">
      <RouterLink
        v-for="item in items"
        :key="item.name"
        :to="{ name: item.name }"
        class="shell__nav"
        :class="{ 'shell__nav--active': isActive(item.name) }"
        :aria-label="item.label"
        :title="item.label"
      >
        <component :is="item.icon" :size="18" :stroke-width="1.75" />
      </RouterLink>
    </nav>

    <aside v-if="$slots.list" class="shell__list island">
      <slot name="list" />
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

.shell__rail {
  display: flex;
  flex: none;
  flex-direction: column;
  gap: var(--space-1);
  align-items: center;
  width: 44px;
  padding-top: var(--space-2);
}

.shell__nav {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  border-radius: var(--radius-md);
  color: var(--color-text-muted);
}

.shell__nav:hover {
  background: var(--color-surface-hover);
  color: var(--color-text);
}

.shell__nav--active {
  background: var(--color-surface-muted);
  color: var(--color-text);
}

.shell__list {
  display: flex;
  flex: none;
  flex-direction: column;
  width: 272px;
  min-height: 0;
  overflow: hidden;
}

.shell__main {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
  overflow: hidden;
}
</style>