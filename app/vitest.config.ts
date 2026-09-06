import { defineConfig } from "vitest/config";

// Separate from vite.config.ts on purpose: the tests are plain modules and file
// reads, so they need neither the React plugin nor a DOM. When Astra's components
// arrive this gains a jsdom environment and a setup file.
export default defineConfig({
  test: {
    environment: "node",
    include: ["src/**/*.test.ts"],
    restoreMocks: true,
  },
});
