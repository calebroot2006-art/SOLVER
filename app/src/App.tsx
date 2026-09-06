import "./App.css";
import { placeholder } from "./placeholder";

/**
 * Static placeholder. It calls nothing: no `invoke`, no plugin, no fetch. The
 * security boundary in app/README.md depends on that staying true until a
 * command is added on purpose and granted in capabilities/default.json.
 */
export default function App() {
  return (
    <main className="shell">
      <h1>{placeholder.title}</h1>
      <ul>
        {placeholder.lines.map((line) => (
          <li key={line}>{line}</li>
        ))}
      </ul>
    </main>
  );
}
