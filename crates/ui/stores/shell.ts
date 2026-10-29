// 窗口外观由插件决定：custom 时用我们的标题栏，native 时退回系统标题栏。
import { defineStore } from "pinia";
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

export type ShellMode = "pending" | "custom" | "native";

export const useShellStore = defineStore("shell", () => {
  const mode = ref<ShellMode>("pending");

  async function activate(): Promise<void> {
    if (!("__TAURI_INTERNALS__" in window)) {
      // 浏览器预览：没有窗口对象，按自绘处理。
      mode.value = "custom";
      return;
    }
    try {
      mode.value = await invoke<"custom" | "native">("activate_and_show");
    } catch {
      mode.value = "native";
    }
  }

  return { mode, activate };
});
