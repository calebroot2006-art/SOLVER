/**
 * The only content the phase 0 shell has. It exists so the window renders
 * something honest, and so the smoke test has real code to check rather than a
 * tautology. Astra replaces all of this with the designed shell.
 */
export interface Placeholder {
  /** Window and heading title. */
  readonly title: string;
  /** One line per fact about where the project currently stands. */
  readonly lines: readonly string[];
}

export const placeholder: Placeholder = {
  title: "GTO Solver APP",
  lines: [
    "Phase 0 shell. No app commands or granted Tauri API permissions.",
    "The solver crates live in crates/. The design lands here next.",
  ],
};
