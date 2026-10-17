import { BrowserRouter, Routes, Route } from "react-router-dom";
import TopBar from "./components/TopBar";
import Home from "./pages/Home";
import ExplorationsIndex from "./pages/explorations/Index";
import LedgerVariants from "./pages/explorations/LedgerVariants";
import ModuleFlowBreadth from "./pages/explorations/ModuleFlowBreadth";
import ModuleFlowDepth from "./pages/explorations/ModuleFlowDepth";
import ModuleFlowVariations from "./pages/explorations/ModuleFlowVariations";
import ReviewIndex from "./pages/review/Index";
import ReactFlowBasics from "./pages/explorations/ReactFlowBasics";
import ThemeSwitchers from "./pages/explorations/ThemeSwitchers";
import { ThemeProvider } from "./theme/ThemeProvider";

/**
 * Shared application shell with routing.
 *
 * Both Tauri and Axum entry points render this component,
 * wrapped in their platform-specific BackendProvider.
 */
export default function App() {
  return (
    <ThemeProvider>
      <BrowserRouter>
        <div className="flex h-screen flex-col">
          <TopBar />
          <div className="min-h-0 flex-1 overflow-auto">
            <Routes>
              <Route path="/" element={<Home />} />
              <Route path="/review" element={<ReviewIndex />} />
              <Route path="/explorations" element={<ExplorationsIndex />} />
              <Route path="/explorations/reactflow-basics" element={<ReactFlowBasics />} />
              <Route path="/explorations/module-flow-breadth" element={<ModuleFlowBreadth />} />
              <Route path="/explorations/module-flow-depth" element={<ModuleFlowDepth />} />
              <Route path="/explorations/module-flow-variations" element={<ModuleFlowVariations />} />
              <Route path="/explorations/ledger-variants" element={<LedgerVariants />} />
              <Route path="/explorations/theme-switchers" element={<ThemeSwitchers />} />
            </Routes>
          </div>
        </div>
      </BrowserRouter>
    </ThemeProvider>
  );
}
