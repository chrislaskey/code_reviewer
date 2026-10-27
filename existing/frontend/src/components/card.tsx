import type { ComponentType, ReactNode } from "react";

/** Any heroicons component. */
export type IconComponent = ComponentType<{ className?: string }>;

/*
 * The card pattern from visualized-elixir: a stacked header with a dark icon
 * disc and a corner status dot, a stats row, and a tab-bar footer.
 *
 * Everything here is presentational. Colour means status and nothing else.
 */

/** The kind's icon on a dark disc, with a status dot on the lower right. */
export function Avatar({
  icon: Icon,
  dotClassName,
  size = "size-10",
}: {
  icon: IconComponent;
  dotClassName: string;
  size?: string;
}) {
  return (
    <span className="relative inline-flex shrink-0">
      <span
        className={`inline-flex items-center justify-center rounded-full bg-base-content text-base-100 ${size}`}
      >
        <Icon className="size-5" />
      </span>
      <span
        className={`absolute -bottom-0.5 -right-0.5 size-3.5 rounded-full ring-2 ring-base-100 ${dotClassName}`}
      />
    </span>
  );
}

/** Long text clipped at the start, so the end of a path stays visible. */
export function StartEllipsis({
  children,
  className = "",
  title,
}: {
  children: string;
  className?: string;
  title?: string;
}) {
  return (
    <p dir="rtl" title={title ?? children} className={`truncate text-left ${className}`}>
      <bdi>{children}</bdi>
    </p>
  );
}

/** Avatar on the left, title above, kind and detail below. */
export function CardHeader({
  icon,
  dotClassName,
  title,
  kindName,
  subtitle,
  subtitleClippedAtStart = false,
  trailing,
}: {
  icon: IconComponent;
  dotClassName: string;
  title: string;
  kindName?: string;
  subtitle?: string;
  /** For paths: clip the beginning, keep the file name. */
  subtitleClippedAtStart?: boolean;
  /** Small content at the right end of the title line. */
  trailing?: ReactNode;
}) {
  const subtitleClass = "font-mono text-[11px] text-base-content/60";
  return (
    <div className="flex h-[60px] items-center gap-3 px-4">
      <Avatar icon={icon} dotClassName={dotClassName} />
      <div className="min-w-0 flex-1">
        <div className="flex items-baseline justify-between gap-2">
          <h3 className="truncate text-sm font-semibold" title={title}>
            {title}
          </h3>
          {trailing}
        </div>
        {subtitleClippedAtStart && subtitle ? (
          <StartEllipsis className={subtitleClass}>{subtitle}</StartEllipsis>
        ) : (
          <p className={`truncate ${subtitleClass}`} title={subtitle}>
            {kindName}
            {kindName && subtitle && <> · </>}
            {subtitle}
          </p>
        )}
      </div>
    </div>
  );
}

export interface Stat {
  label: string;
  value: ReactNode;
  className?: string;
}

/** Small labelled readings in a row. */
export function StatsRow({ stats }: { stats: Stat[] }) {
  return (
    <div className="flex h-[44px] gap-6 px-4 text-sm">
      {stats.map((stat) => (
        <div key={stat.label} className={stat.className}>
          <div className="text-[10px] uppercase tracking-wide text-base-content/40">
            {stat.label}
          </div>
          <div className="font-mono">{stat.value}</div>
        </div>
      ))}
    </div>
  );
}

export interface Tab {
  id: string;
  label: string;
  icon: IconComponent;
}

/** The footer: every label visible, the active tab darker. */
export function TabBar({
  tabs,
  active,
  onSelect,
}: {
  tabs: Tab[];
  active: string | null;
  onSelect: (id: string) => void;
}) {
  return (
    <div
      className="flex overflow-hidden rounded-b-xl bg-stone-200/40 dark:bg-stone-800/40"
      role="tablist"
    >
      {tabs.map(({ id, label, icon: Icon }) => {
        const selected = id === active;
        return (
          <button
            key={id}
            type="button"
            role="tab"
            aria-selected={selected}
            onClick={() => onSelect(id)}
            className={[
              "nodrag nopan flex flex-1 flex-col items-center gap-0.5 py-1.5 text-[10px]",
              selected
                ? "font-semibold text-base-content"
                : "text-base-content/50 hover:text-base-content",
            ].join(" ")}
          >
            <Icon className="size-4" />
            {label}
          </button>
        );
      })}
    </div>
  );
}

/** The rounded bordered shell every card sits in. */
export function Card({
  className = "",
  children,
}: {
  className?: string;
  children: ReactNode;
}) {
  return (
    <div
      className={`relative rounded-xl border border-base-300 bg-base-100 shadow-sm ${className}`}
    >
      {children}
    </div>
  );
}
