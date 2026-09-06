import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import App from "./App";
import { placeholder } from "./placeholder";

const appDir = fileURLToPath(new URL("..", import.meta.url));

function readAppFile(relativePath: string): string {
  return readFileSync(join(appDir, relativePath), "utf8");
}

// Drop Rust line and block comments, so a check for a construct is not satisfied by
// prose that merely names it. Good enough for the two small files below, and not a
// Rust parser.
function rustCode(source: string): string {
  return source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/^\s*\/\/.*$/gm, "");
}

// The same for TOML's `#` comments.
function tomlCode(source: string): string {
  return source.replace(/^\s*#.*$/gm, "");
}

describe("placeholder shell", () => {
  it("has something to render", () => {
    expect(placeholder.title).toBe("GTO Solver APP");
    expect(placeholder.lines.length).toBeGreaterThan(0);
    for (const line of placeholder.lines) {
      expect(line.trim()).not.toBe("");
    }
  });

  it("renders that content", () => {
    // Static markup rather than a DOM: it exercises the real component tree with
    // no extra dependency. It is not proof the WebView shows it, which is the
    // release-build check.
    const markup = renderToStaticMarkup(createElement(App));
    expect(markup).toContain(placeholder.title);
    for (const line of placeholder.lines) {
      expect(markup).toContain(line);
    }
  });
});

// The rest of this file guards the scaffold security boundary that Astra's P01
// finding asked for. These are the checks that fail first if a plugin, a command,
// or a remote origin is added without the matching review. They read the files
// rather than the running app, so they say nothing about runtime behaviour: the
// release build and the ungranted-command check are Astra's to run.
describe("scaffold security boundary", () => {
  it("declares no Tauri plugin package", () => {
    const manifest: unknown = JSON.parse(readAppFile("package.json"));
    const { dependencies = {}, devDependencies = {} } = manifest as {
      dependencies?: Record<string, string>;
      devDependencies?: Record<string, string>;
    };
    const named = [...Object.keys(dependencies), ...Object.keys(devDependencies)];
    expect(named.filter((name) => name.startsWith("@tauri-apps/plugin-"))).toEqual([]);
  });

  it("registers no command and no plugin in the Rust shell", () => {
    const shell = rustCode(readAppFile("src-tauri/src/lib.rs"));
    expect(shell).not.toContain("invoke_handler");
    expect(shell).not.toContain("tauri::command");
    expect(shell).not.toContain(".plugin(");

    const cargoToml = tomlCode(readAppFile("src-tauri/Cargo.toml"));
    expect(cargoToml).not.toContain("tauri-plugin-");
  });

  it("calls nothing over the Tauri bridge from the frontend", () => {
    for (const source of ["src/App.tsx", "src/main.tsx", "src/placeholder.ts"]) {
      const text = readAppFile(source);
      expect(text).not.toContain("@tauri-apps/api");
      expect(text).not.toContain("invoke(");
      expect(text).not.toContain("fetch(");
    }
  });

  it("selects only the empty local main-window capability", () => {
    const capability = JSON.parse(
      readAppFile("src-tauri/capabilities/default.json"),
    ) as {
      local?: boolean;
      remote?: unknown;
      windows?: string[];
      permissions?: string[];
    };
    const config = JSON.parse(readAppFile("src-tauri/tauri.conf.json")) as {
      app?: {
        windows?: { label?: string; url?: string }[];
        security?: { capabilities?: unknown };
      };
    };

    expect(capability.local).toBe(true);
    expect(capability.remote).toBeUndefined();
    expect(capability.windows).toEqual(["main"]);
    expect(capability.permissions).toEqual([]);
    // Tauri otherwise enables every capability file in the directory. An added
    // file must not silently become another grant for this window.
    expect(config.app?.security?.capabilities).toEqual(["default"]);
    expect(config.app?.windows).toHaveLength(1);
    expect(config.app?.windows?.[0]?.label).toBe("main");
    expect(config.app?.windows?.[0]?.url).toBeUndefined();
  });

  it("ships a production CSP that allows bundled assets and the IPC origin only", () => {
    const config = JSON.parse(readAppFile("src-tauri/tauri.conf.json")) as {
      app?: { security?: { csp?: Record<string, string> | string | null } };
    };
    const csp = config.app?.security?.csp;

    // Exact directives guard schemes, host boundaries, missing directives, and
    // extra source types. A hostname-prefix regex would admit ipc.localhost.evil
    // and would miss a scheme-only source such as https: in img-src.
    expect(csp).toEqual({
      "default-src": "'self'",
      "script-src": "'self'",
      "style-src": "'self'",
      "img-src": "'self' data:",
      "font-src": "'self'",
      "connect-src": "'self' ipc: http://ipc.localhost",
      "object-src": "'none'",
      "base-uri": "'self'",
      "form-action": "'none'",
      "frame-ancestors": "'none'",
    });
  });

  it("keeps Vite and HMR allowances in the development CSP", () => {
    const config = JSON.parse(readAppFile("src-tauri/tauri.conf.json")) as {
      app?: { security?: { devCsp?: unknown } };
    };
    expect(config.app?.security?.devCsp).toEqual({
      "default-src": "'self'",
      "script-src": "'self' 'unsafe-inline' http://localhost:1420",
      "style-src": "'self' 'unsafe-inline' http://localhost:1420",
      "img-src": "'self' data: http://localhost:1420",
      "font-src": "'self' data: http://localhost:1420",
      "connect-src":
        "'self' ipc: http://ipc.localhost http://localhost:1420 ws://localhost:1420 ws://localhost:1421",
      "object-src": "'none'",
      "base-uri": "'self'",
      "form-action": "'none'",
      "frame-ancestors": "'none'",
    });
  });
});
