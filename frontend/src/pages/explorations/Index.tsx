import { Link } from "react-router-dom";

const EXPLORATIONS = [
  {
    path: "/explorations/reactflow-basics",
    title: "ReactFlow basics",
    note: "ReactFlow with custom card nodes and hardcoded data, full page. Checks that the library renders the same in the Tauri and Axum apps.",
  },
  {
    path: "/explorations/module-flow-breadth",
    title: "Module flow, breadth",
    note: "Inputs → file → outputs drawn nine different ways with the same hardcoded example, to see which ideas are worth refining.",
  },
  {
    path: "/explorations/module-flow-depth",
    title: "Module flow, depth",
    note: "The promising ideas from the breadth round, combined and made interactive: focus a function, expand a group, jump between files.",
  },
  {
    path: "/explorations/module-flow-variations",
    title: "Module flow, variations",
    note: "A third round: the E3 ledger with pieces from the ledger variants, and the A1 and A2 node views with one connection point, hover to show connections, and no stats row.",
  },
  {
    path: "/explorations/ledger-variants",
    title: "Ledger variants",
    note: "Ledgers without the flow diagram: columns File / Outputs / Inputs, by function as lists or pills, and an attention list across files.",
  },
  {
    path: "/explorations/groups-breadth",
    title: "Groups, breadth",
    note: "The top level of a grouped review drawn six ways (cards, table, sections, tree, dependency matrix, filters), plus the opt-in states and two ways to define groups.",
  },
  {
    path: "/explorations/groups-depth",
    title: "Groups, depth",
    note: "Moving across levels: groups and files at the top, subgroups in the middle, the file and function view at the bottom. Drill down, tree and levels, and expand in place.",
  },
  {
    path: "/explorations/groups-flow",
    title: "Groups, flow",
    note: "Two levels instead of three: the Groups view is the file view one level up (group in the header, files as content), and a file click opens the Functions view. Ledger and node renderers, three ways to navigate.",
  },
  {
    path: "/explorations/theme-switchers",
    title: "Theme switchers",
    note: "Six live ways to switch between system, light and dark.",
  },
];

export default function ExplorationsIndex() {
  return (
    <main className="mx-auto max-w-4xl space-y-6 px-6 py-8">
      <header className="space-y-2">
        <p className="text-sm">
          <Link to="/" className="text-primary hover:underline">
            ← Home
          </Link>
        </p>
        <h1 className="text-2xl font-bold">Explorations</h1>
        <p className="max-w-2xl text-sm text-base-content/70">
          Scratch pages where one piece of the interface is drawn several ways at once, so a
          direction can be picked by looking at it. Everything on them is hardcoded and nothing
          is wired to the backend.
        </p>
      </header>

      <ul className="divide-y divide-base-300 border-y border-base-300">
        {EXPLORATIONS.map((exploration) => (
          <li key={exploration.path}>
            <Link
              to={exploration.path}
              className="group flex items-baseline justify-between gap-6 py-4 hover:bg-base-200/60"
            >
              <div className="space-y-1">
                <h2 className="font-semibold group-hover:text-primary">{exploration.title}</h2>
                <p className="max-w-2xl text-sm text-base-content/60">{exploration.note}</p>
              </div>
              <span className="font-mono text-xs text-base-content/40">{exploration.path}</span>
            </Link>
          </li>
        ))}
      </ul>
    </main>
  );
}
