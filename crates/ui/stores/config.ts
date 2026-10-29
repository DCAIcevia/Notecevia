// 应用配置：跟着程序走，与工作区无关。启动时读一次，改了就写回。

import { defineStore } from "pinia";
import { ref } from "vue";
import { api } from "../api/client";
import type { AppConfig, AppConfigSnapshot, IpcFailure } from "../api/types";

export const useConfigStore = defineStore("config", () => {
  const snapshot = ref<AppConfigSnapshot | null>(null);
  const failure = ref<IpcFailure | null>(null);

  /** 读配置。读不到就用调用方给的默认值，不挡界面。 */
  async function load(): Promise<void> {
    failure.value = null;
    try {
      snapshot.value = await api.appConfig();
    } catch (error) {
      failure.value = error as IpcFailure;
    }
  }

  async function save(config: AppConfig): Promise<boolean> {
    failure.value = null;
    try {
      await api.saveAppConfig(config);
      snapshot.value = snapshot.value
        ? { path: snapshot.value.path, config }
        : { path: "", config };
      return true;
    } catch (error) {
      failure.value = error as IpcFailure;
      return false;
    }
  }

  return { snapshot, failure, load, save };
});
