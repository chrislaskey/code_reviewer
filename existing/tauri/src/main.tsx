import React from "react";
import ReactDOM from "react-dom/client";
import { App, BackendProvider } from "@code-reviewer/ui";
import { tauriAdapter } from "./adapters/tauri";
import "./styles.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <BackendProvider value={tauriAdapter}>
      <App />
    </BackendProvider>
  </React.StrictMode>,
);
