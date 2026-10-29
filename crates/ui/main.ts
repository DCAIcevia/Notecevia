import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import { router } from "./router";
import { useConfigStore } from "./stores/config";
import "./theme/default.css";
import "./theme/base.css";

const pinia = createPinia();
createApp(App).use(pinia).use(router).mount("#app");

// 开发期在左下角显示启动耗时，录屏或截图都能直接读到：
// 脚本执行到挂载用了多久、列表真正出来又用了多久。
if (import.meta.env.DEV) {
  const boot = (window as unknown as { __BOOT_AT?: number }).__BOOT_AT ?? 0;
  const mounted = performance.now();
  const badge = document.createElement("div");
  badge.style.cssText =
    "position:fixed;left:8px;bottom:6px;z-index:99999;font:11px/1.4 ui-monospace,monospace;color:#66665f;background:#f5f5f3cc;padding:2px 6px;border-radius:4px;pointer-events:none";
  badge.textContent = `脚本 ${Math.round(mounted)}ms · 挂载完成`;
  document.body.appendChild(badge);

  const timer = window.setInterval(() => {
    if (document.querySelector(".home__list li, .home__state")) {
      badge.textContent = `首屏 ${Math.round(performance.now() - boot)}ms（脚本 ${Math.round(mounted)}ms）`;
      window.clearInterval(timer);
    }
  }, 50);
}

// 配置读不到不挡界面，缺省值照常工作。
void useConfigStore(pinia).load();