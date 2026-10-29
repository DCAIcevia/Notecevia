import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    environment: "jsdom",
    include: ["crates/ui/**/*.spec.ts"],
  },
});
