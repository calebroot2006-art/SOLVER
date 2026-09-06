import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";

const root = document.getElementById("root");
if (!root) {
  // Loud rather than a blank window: if this ever fires, index.html and this file
  // have drifted apart and the shell has no mount point.
  throw new Error("index.html is missing the #root element the app mounts into");
}

ReactDOM.createRoot(root).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
