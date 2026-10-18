import { useEffect, useRef, useState, type ReactNode } from "react";

/*
 * A file tree beside a long page, like a GitHub pull request. Clicking an item
 * scrolls the page to its section instead of hiding the others, and the item for
 * the section on screen is highlighted. The tree stays put while the page scrolls.
 */

export interface TreeItem {
  id: string;
  label: string;
  depth: number;
  icon?: ReactNode;
  /** Small text at the right end, like a file count. */
  detail?: string;
  /** Quieter, for "Ungrouped". */
  dim?: boolean;
  /** Files: monospace and smaller. */
  mono?: boolean;
}

/** The anchor name for a tree item inside one page. */
export function anchorId(prefix: string, id: string): string {
  return `${prefix}${id.replace(/[^A-Za-z0-9_-]/g, "_")}`;
}

/** Scroll the page so the element is near the top, with a little room. */
export function scrollToElement(element: Element | null) {
  if (!element) return;
  (element as HTMLElement).style.scrollMarginTop = "16px";
  element.scrollIntoView({ behavior: "smooth", block: "start" });
}

export function ScrollTreeLayout({
  prefix,
  items,
  resolve,
  scrollTarget,
  beforeJump,
  watchKey,
  heading,
  children,
}: {
  /** Makes the anchors unique when several trees are on one page. */
  prefix: string;
  items: TreeItem[];
  /** The element an item scrolls to. Defaults to the one whose id is the item's anchor. */
  resolve?: (item: TreeItem) => Element | null;
  /** Where a click scrolls, when that differs from the element the item watches. */
  scrollTarget?: (item: TreeItem) => Element | null;
  /** Runs before scrolling, to open whatever the item sits inside. */
  beforeJump?: (item: TreeItem) => void;
  /** Changes when sections open or close, so the highlight watches the right elements. */
  watchKey?: string;
  heading?: ReactNode;
  children: ReactNode;
}) {
  const [activeId, setActiveId] = useState<string | null>(items[0]?.id ?? null);
  const find = (item: TreeItem) =>
    resolve ? resolve(item) : document.getElementById(anchorId(prefix, item.id));
  const findRef = useRef(find);
  findRef.current = find;
  const targetRef = useRef(scrollTarget ?? find);
  targetRef.current = scrollTarget ?? find;

  useEffect(() => {
    const visible = new Set<string>();
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          const id = (entry.target as HTMLElement).dataset.treeItem;
          if (!id) continue;
          if (entry.isIntersecting) visible.add(id);
          else visible.delete(id);
        }
        const first = items.find((item) => visible.has(item.id));
        if (first) setActiveId(first.id);
      },
      { rootMargin: "-5% 0px -75% 0px" },
    );
    for (const item of items) {
      const element = findRef.current(item);
      if (!element) continue;
      (element as HTMLElement).dataset.treeItem = item.id;
      observer.observe(element);
    }
    return () => observer.disconnect();
    // `watchKey` stands for the sections that are open.
  }, [items, watchKey]);

  const jump = (item: TreeItem) => {
    setActiveId(item.id);
    beforeJump?.(item);
    // Wait for opened sections to render before measuring.
    window.setTimeout(() => scrollToElement(targetRef.current(item)), 60);
  };

  return (
    <div className="flex items-start bg-base-100">
      <aside className="sticky top-4 max-h-[calc(100vh-6rem)] w-60 shrink-0 self-start overflow-y-auto border-r border-base-300 py-2">
        {heading}
        <nav>
          {items.map((item) => (
            <a
              key={item.id}
              href={`#${anchorId(prefix, item.id)}`}
              onClick={(event) => {
                event.preventDefault();
                jump(item);
              }}
              style={{ paddingLeft: 12 + item.depth * 14 }}
              className={[
                "flex items-center gap-1.5 py-1 pr-3 text-xs",
                item.id === activeId ? "bg-primary/10 font-semibold" : "hover:bg-base-200/70",
                item.dim ? "text-base-content/60" : "",
                item.mono ? "font-mono text-[11px]" : "",
              ].join(" ")}
            >
              {item.icon && <span className="shrink-0 text-base-content/50">{item.icon}</span>}
              <span className="min-w-0 flex-1 truncate" title={item.label}>
                {item.label}
              </span>
              {item.detail && (
                <span className="shrink-0 font-mono text-[10px] text-base-content/40">
                  {item.detail}
                </span>
              )}
            </a>
          ))}
        </nav>
      </aside>
      <div className="min-w-0 flex-1">{children}</div>
    </div>
  );
}
