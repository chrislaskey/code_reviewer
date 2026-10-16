import { createContext, useContext } from "react";
import type { BackendAdapter } from "./adapter";

const BackendContext = createContext<BackendAdapter | null>(null);

/** Provide a BackendAdapter to all child components. */
export const BackendProvider = BackendContext.Provider;

/** Access the BackendAdapter from any component. */
export function useBackend(): BackendAdapter {
  const adapter = useContext(BackendContext);
  if (!adapter) {
    throw new Error("useBackend must be used within a BackendProvider");
  }
  return adapter;
}
