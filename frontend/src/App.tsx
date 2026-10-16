import { BrowserRouter, Routes, Route } from "react-router-dom";
import Home from "./pages/Home";

/**
 * Shared application shell with routing.
 *
 * Both Tauri and Axum entry points render this component,
 * wrapped in their platform-specific BackendProvider.
 */
export default function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/" element={<Home />} />
      </Routes>
    </BrowserRouter>
  );
}
