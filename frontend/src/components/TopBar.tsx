import { Link } from "react-router-dom";
import { ThemeSwitcher } from "../theme/ThemeSwitchers";

/** The strip across the top of every page. */
export default function TopBar() {
  return (
    <header className="flex shrink-0 items-center gap-5 border-b border-base-300 bg-base-100 px-4 py-2">
      <Link to="/" className="text-sm font-semibold">
        Code Reviewer
      </Link>
      <nav className="flex gap-4 text-xs text-base-content/60">
        <Link to="/review" className="transition-colors hover:text-base-content">
          Review (ReactFlow)
        </Link>
        <Link to="/review/html" className="transition-colors hover:text-base-content">
          Review (HTML)
        </Link>
        <Link to="/explorations" className="transition-colors hover:text-base-content">
          Explorations
        </Link>
      </nav>
      <div className="ml-auto">
        <ThemeSwitcher />
      </div>
    </header>
  );
}
