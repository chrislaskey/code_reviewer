import React from "react";
import ReactDOM from "react-dom/client";
import { App, BackendProvider } from "@code-reviewer/ui";
import { httpAdapter } from "./adapters/http";
import "./styles.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <BackendProvider value={httpAdapter}>
      <App />
    </BackendProvider>
  </React.StrictMode>,
);
