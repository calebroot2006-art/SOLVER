import js from "@eslint/js";
import globals from "globals";
import reactHooks from "eslint-plugin-react-hooks";
import reactRefresh from "eslint-plugin-react-refresh";
import tseslint from "typescript-eslint";

export default tseslint.config(
  {
    ignores: ["dist", "src-tauri/target", "src-tauri/gen"],
  },
  js.configs.recommended,
  tseslint.configs.recommended,
  // The `flat` namespace is the one whose `plugins` key is an object; the
  // top-level `configs["recommended-latest"]` is still eslintrc-shaped.
  reactHooks.configs.flat["recommended-latest"],
  {
    files: ["src/**/*.{ts,tsx}"],
    languageOptions: {
      ecmaVersion: 2022,
      globals: globals.browser,
    },
    plugins: { "react-refresh": reactRefresh },
    rules: {
      "react-refresh/only-export-components": ["warn", { allowConstantExport: true }],
    },
  },
  {
    // Config files and the tests run in Node, not in the webview.
    files: ["*.config.ts", "src/**/*.test.ts"],
    languageOptions: { globals: globals.node },
    rules: { "react-refresh/only-export-components": "off" },
  },
);
