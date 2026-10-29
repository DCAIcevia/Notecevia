import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
// @ts-expect-error type error without @types/node package
import process from "node:process";
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(() => ({
  plugins: [vue()],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    // 绑 IPv4 回环：默认的 localhost 在本机会解析到 ::1，而系统的代理绕过列表
    // 只写了 localhost 与 127.*，WebView2 打开 ::1 时会被代理接走，首屏要多等十几秒。
    host: host || "127.0.0.1",
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    // 启动时先把界面源码转换一遍：窗口加载时这些模块已经是热的，
    // 用户看到的白屏少一节。代价放在服务器启动阶段，那时还没有窗口。
    warmup: {
      clientFiles: ["./crates/ui/main.ts", "./crates/ui/**/*.vue", "./crates/ui/**/*.ts"],
    },
    watch: {
      // 3. 窗口壳在 crates/app，不跟着它重启前端
      // 4. target 里是 Rust 构建产物（三万个文件），盯它只会白烧 CPU
      ignored: ["**/crates/app/**", "**/target/**", "**/.agents/**", "**/docs/**"],
    },
  },
}));