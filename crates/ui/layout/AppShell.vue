<script setup lang="ts">
// 应用骨架：导航栏 + 可选的次级列表栏 + 主内容。
// 导航栏常驻，切区域只换列表栏与主内容；画板与画布共用同一套列表面板。
import { useRoute } from "vue-router";
import { Grid, Trash } from "../icons";
import TitleBar from "./TitleBar.vue";
import { getCurrentWindow } from "@tauri-apps/api/window";

const props = withDefaults(defineProps<{ bare?: boolean }>(), { bare: false });
const route = useRoute();

const primary = [{ name: "home", label: "首页", icon: Grid }] as const;

// 回收站是低频动作，压在导航栏底部，不跟日常导航混在一起。
const secondary = { name: "trash", label: "回收站", icon: Trash } as const;

type Direction = "North" | "South" | "East" | "West" | "NorthEast" | "NorthWest" | "SouthEast" | "SouthWest";

/** 无边框窗口没有系统边框，四条边与四角靠这里交给窗口去拖。 */
function resize(direction: Direction): void {
  void getCurrentWindow()
    .startResizeDragging(direction)
    .catch(() => {
      // 浏览器预览里没有窗口对象。
    });
}

function isActive(name: string): boolean {
  const current = String(route.name);
  if (name === "home") return current === "home" || current === "volume";
  return current === name;
}
</script>

<template>
  <div class="window">
    <TitleBar />
    <main class="shell">
    <nav class="shell__rail">
      <RouterLink
        v-for="item in primary"
        :key="item.name"
        :to="{ name: item.name }"
        class="shell__nav"
        :class="{ 'shell__nav--active': isActive(item.name) }"
        :aria-label="item.label"
        :title="item.label"
      >
        <component :is="item.icon" :size="18" :stroke-width="1.75" />
      </RouterLink>

      <span class="shell__spacer" />

      <RouterLink
        :to="{ name: secondary.name }"
        class="shell__nav"
        :class="{ 'shell__nav--active': isActive(secondary.name) }"
        :aria-label="secondary.label"
        :title="secondary.label"
      >
        <component :is="secondary.icon" :size="18" :stroke-width="1.75" />
      </RouterLink>
    </nav>

    <aside v-if="$slots.list" class="shell__list island">
      <slot name="list" />
    </aside>

    <section class="shell__main" :class="{ island: !props.bare }">
      <slot />
      </section>
    </main>

    <!-- 无边框窗口的四条边：只有最外圈几像素响应，用作拖拽改大小 -->
    <span class="grip grip--top" @pointerdown="resize('North')" />
    <span class="grip grip--bottom" @pointerdown="resize('South')" />
    <span class="grip grip--left" @pointerdown="resize('West')" />
    <span class="grip grip--right" @pointerdown="resize('East')" />
    <span class="grip grip--nw" @pointerdown="resize('NorthWest')" />
    <span class="grip grip--ne" @pointerdown="resize('NorthEast')" />
    <span class="grip grip--sw" @pointerdown="resize('SouthWest')" />
    <span class="grip grip--se" @pointerdown="resize('SouthEast')" />
  </div>
</template>

<style scoped>
.window {
  position: relative;
  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--color-surface-app);
}

.shell {
  display: flex;
  flex: 1;
  gap: var(--space-2);
  min-height: 0;
  padding: 0 var(--space-2) var(--space-2);
}

/* 无边框窗口的拖拽改大小区：只有最外圈 4 像素，压在内容之上 */
.grip {
  position: absolute;
  z-index: 10;
}

.grip--top,
.grip--bottom {
  right: 8px;
  left: 8px;
  height: 4px;
  cursor: ns-resize;
}

.grip--top {
  top: 0;
}

.grip--bottom {
  bottom: 0;
}

.grip--left,
.grip--right {
  top: 8px;
  bottom: 8px;
  width: 4px;
  cursor: ew-resize;
}

.grip--left {
  left: 0;
}

.grip--right {
  right: 0;
}

.grip--nw,
.grip--ne,
.grip--sw,
.grip--se {
  width: 8px;
  height: 8px;
}

.grip--nw {
  top: 0;
  left: 0;
  cursor: nwse-resize;
}

.grip--ne {
  top: 0;
  right: 0;
  cursor: nesw-resize;
}

.grip--sw {
  bottom: 0;
  left: 0;
  cursor: nesw-resize;
}

.grip--se {
  right: 0;
  bottom: 0;
  cursor: nwse-resize;
}

.shell__rail {
  display: flex;
  flex: none;
  flex-direction: column;
  gap: var(--space-1);
  align-items: center;
  width: 44px;
  padding: var(--space-2) 0;
}

.shell__spacer {
  flex: 1;
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

/* bare 模式留给画布页：画布自己就是一块岛，外面不能再套一层白底，
   否则岛与岛之间的缝会被这层白填掉，看起来就只剩一块。 */
.shell__main:not(.island) {
  background: none;
  box-shadow: none;
}
</style>