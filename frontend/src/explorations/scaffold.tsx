import type { ReactNode } from "react";
import { Link } from "react-router-dom";

/* Pieces every exploration page uses, as in visualized-elixir's explorations. */

export function ExplorationPage({
  title,
  intro,
  children,
}: {
  title: string;
  intro: ReactNode;
  children: ReactNode;
}) {
  return (
    <main className="mx-auto max-w-[1400px] space-y-10 px-6 py-8">
      <header className="space-y-2">
        <p className="text-sm">
          <Link to="/explorations" className="text-primary hover:underline">
            ← Explorations
          </Link>
        </p>
        <h1 className="text-2xl font-bold">{title}</h1>
        <div className="max-w-3xl space-y-2 text-sm text-base-content/70">{intro}</div>
      </header>
      {children}
    </main>
  );
}

export function Section({
  id,
  title,
  intro,
  children,
}: {
  id: string;
  title: string;
  intro?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section id={id} className="scroll-mt-6 space-y-4">
      <header className="space-y-1 border-b border-base-300 pb-2">
        <h2 className="text-lg font-semibold">{title}</h2>
        {intro && <p className="max-w-3xl text-sm text-base-content/70">{intro}</p>}
      </header>
      <div className="space-y-6">{children}</div>
    </section>
  );
}

/** One option: a label, the drawing on a dotted background, and a note on what it tries. */
export function Option({
  code,
  title,
  note,
  children,
}: {
  code: string;
  title: string;
  note: ReactNode;
  children: ReactNode;
}) {
  return (
    <article className="overflow-hidden rounded-xl border border-base-300">
      <header className="flex items-baseline gap-3 border-b border-base-300 bg-base-100 px-4 py-2.5">
        <span className="font-mono text-xs text-primary">{code}</span>
        <h3 className="text-sm font-semibold">{title}</h3>
      </header>
      <div className="bg-base-200/40">{children}</div>
      <p className="border-t border-base-300 bg-base-100 px-4 py-2.5 text-xs leading-relaxed text-base-content/70">
        {note}
      </p>
    </article>
  );
}

/** Legend for the markers and colours the marked-up options use. */
export function Legend({ items }: { items: { swatch: ReactNode; label: string }[] }) {
  return (
    <ul className="flex flex-wrap gap-x-5 gap-y-1 border-t border-base-300 bg-base-100 px-4 py-2 text-[11px] text-base-content/70">
      {items.map((item) => (
        <li key={item.label} className="flex items-center gap-1.5">
          {item.swatch}
          {item.label}
        </li>
      ))}
    </ul>
  );
}
