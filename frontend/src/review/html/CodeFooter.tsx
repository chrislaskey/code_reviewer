import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import { ChevronRightIcon } from "@heroicons/react/16/solid";
import type {
  DiffLine,
  DiffLineKind,
  FileDiff,
  FileDiffTarget,
  FunctionDiff,
  FunctionDiffTarget,
} from "../../api/types";
import {
  functionKey,
  type ChangedFunction,
  type ExampleFile,
} from "../../explorations/data";
import { useOpenAllState } from "../../explorations/open_all";
import { CHANGE_STYLES } from "../../explorations/style";

/*
 * The code under a ledger. A parsed file shows the whole of each changed
 * function. Any other file shows its changed lines with a few lines of context.
 * Changed lines are marked. The code comes from the backend, one request per
 * file. Without loaders (the design explorations) nothing is shown.
 */

export interface CodeLoaders {
  loadFunctionDiffs: (targets: FunctionDiffTarget[]) => Promise<FunctionDiff[]>;
  loadFileDiffs: (targets: FileDiffTarget[]) => Promise<FileDiff[]>;
}

export const CodeLoadersContext = createContext<CodeLoaders | null>(null);

/** True when loaders are provided, so the ledger should show a code footer. */
export function useHasCodeLoaders(): boolean {
  return useContext(CodeLoadersContext) !== null;
}

/**
 * How the code of a parsed file is laid out. `per_function`: a closed block for
 * each function. `one_block`: one container for all of them, and hovering a
 * function shows only its code.
 */
export type CodeLayout = "per_function" | "one_block";

const LINE_STYLES: Record<DiffLineKind, { row: string; sign: string; signText: string }> = {
  context: { row: "", sign: " ", signText: "" },
  added: {
    row: "bg-emerald-500/15",
    sign: "+",
    signText: "text-emerald-600 dark:text-emerald-400",
  },
  removed: {
    row: "bg-rose-500/15",
    sign: "−",
    signText: "text-rose-600 dark:text-rose-400",
  },
};

type Loaded<T> =
  | { state: "loading" }
  | { state: "failed"; message: string }
  | { state: "ready"; items: T[] };

/** Runs `load` when it changes. `load` null loads nothing. */
function useLoaded<T>(load: (() => Promise<T[]>) | null): Loaded<T> {
  const [loaded, setLoaded] = useState<Loaded<T>>({ state: "loading" });

  useEffect(() => {
    if (!load) return;
    let outdated = false;
    setLoaded({ state: "loading" });
    load()
      .then((items) => {
        if (!outdated) setLoaded({ state: "ready", items });
      })
      .catch((e) => {
        if (!outdated) {
          setLoaded({ state: "failed", message: e instanceof Error ? e.message : String(e) });
        }
      });
    return () => {
      outdated = true;
    };
  }, [load]);

  return loaded;
}

function countLines(lines: DiffLine[], kind: DiffLineKind): number {
  return lines.filter((line) => line.kind === kind).length;
}

function LineCounts({ lines }: { lines: DiffLine[] }) {
  const added = countLines(lines, "added");
  const removed = countLines(lines, "removed");
  return (
    <span className="ml-auto flex gap-2 text-[11px]">
      {added > 0 && <span className="text-emerald-600 dark:text-emerald-400">+{added}</span>}
      {removed > 0 && <span className="text-rose-600 dark:text-rose-400">−{removed}</span>}
    </span>
  );
}

function Gutter({ number }: { number: number | null }) {
  return (
    <span className="w-9 shrink-0 select-none pr-2 text-right text-base-content/35">
      {number ?? ""}
    </span>
  );
}

function CodeLine({ line }: { line: DiffLine }) {
  const style = LINE_STYLES[line.kind];
  return (
    <div className={`flex ${style.row}`}>
      <Gutter number={line.old_number} />
      <Gutter number={line.new_number} />
      <span className={`w-4 shrink-0 select-none text-center font-bold ${style.signText}`}>
        {style.sign}
      </span>
      <span className="whitespace-pre pr-4">{line.text}</span>
    </div>
  );
}

/** True when lines were left out between the two, judged by their line numbers. */
function hasGapBetween(previous: DiffLine | undefined, line: DiffLine): boolean {
  if (!previous) return false;
  const jumped = (before: number | null, after: number | null) =>
    before !== null && after !== null && after > before + 1;
  return jumped(previous.old_number, line.old_number) || jumped(previous.new_number, line.new_number);
}

function CodeLines({ lines }: { lines: DiffLine[] }) {
  return (
    <div className="nodrag nopan overflow-x-auto pb-2 font-mono text-[11px] leading-5">
      <div className="w-max min-w-full">
        {lines.map((line, index) => (
          <div key={index}>
            {hasGapBetween(lines[index - 1], line) && (
              <div className="select-none bg-base-content/5 pl-20 text-base-content/35">⋯</div>
            )}
            <CodeLine line={line} />
          </div>
        ))}
      </div>
    </div>
  );
}

function ChevronOpenIcon({ open }: { open: boolean }) {
  return (
    <ChevronRightIcon
      className={`size-3.5 text-base-content/50 transition-transform ${open ? "rotate-90" : ""}`}
    />
  );
}

const SECTION_BUTTON =
  "nodrag nopan flex h-8 w-full cursor-pointer items-center gap-2 px-3 text-left font-mono text-xs";

function FooterFrame({ children }: { children: ReactNode }) {
  return (
    <div className="border-t border-base-content/10">
      <div className="flex h-5 items-center px-3 pt-2 text-[10px] uppercase tracking-wide text-base-content/40">
        Code
      </div>
      {children}
    </div>
  );
}

function LoadMessage({ loaded }: { loaded: Loaded<unknown> }) {
  if (loaded.state === "loading") {
    return <p className="px-3 py-2 font-mono text-[11px] text-base-content/50">Loading code…</p>;
  }
  if (loaded.state === "failed") {
    return (
      <p className="px-3 py-2 font-mono text-[11px] text-rose-700 dark:text-rose-300">
        {loaded.message}
      </p>
    );
  }
  return null;
}

/* -- Parsed files: the whole of each changed function ------------------------- */

function FunctionHeading({ change, diff }: { change: ChangedFunction; diff: FunctionDiff | undefined }) {
  const style = CHANGE_STYLES[change.kind];
  return (
    <>
      <span className={`w-3 text-center font-bold ${style.text}`}>{style.sign}</span>
      <span className="truncate">
        {change.name}/{change.arity}
        {change.previous_name && (
          <span className="text-base-content/40"> ← {change.previous_name}</span>
        )}
      </span>
      <LineCounts lines={diff?.lines ?? []} />
    </>
  );
}

function FunctionCode({
  change,
  diff,
  hovered,
}: {
  change: ChangedFunction;
  diff: FunctionDiff | undefined;
  hovered: boolean;
}) {
  const [pinned, setPinned] = useOpenAllState(false);
  const open = pinned || hovered;
  return (
    <div className="border-t border-base-content/10 first:border-t-0">
      <button
        type="button"
        onClick={() => setPinned((current) => !current)}
        className={`${SECTION_BUTTON} ${hovered ? "bg-base-content/5" : ""}`}
      >
        <ChevronOpenIcon open={open} />
        <FunctionHeading change={change} diff={diff} />
      </button>
      {open && <CodeLines lines={diff?.lines ?? []} />}
    </div>
  );
}

/** One container for the code of every function. Click to open or close all of it. */
function OneBlockCode({
  changes,
  diffs,
  hoveredKey,
}: {
  changes: ChangedFunction[];
  diffs: FunctionDiff[];
  hoveredKey: string | null;
}) {
  const [pinned, setPinned] = useOpenAllState(false);
  const hoveredIndex = changes.findIndex((change) => functionKey(change) === hoveredKey);
  const previewing = hoveredIndex >= 0;
  const open = pinned || previewing;
  const shown = previewing ? [hoveredIndex] : changes.map((_, index) => index);
  return (
    <div>
      <button
        type="button"
        onClick={() => setPinned((current) => !current)}
        className={`${SECTION_BUTTON} ${previewing ? "bg-base-content/5" : ""}`}
      >
        <ChevronOpenIcon open={open} />
        <span className="text-base-content/70">
          {changes.length} changed {changes.length === 1 ? "function" : "functions"}
        </span>
      </button>
      {open &&
        shown.map((index) => (
          <div key={functionKey(changes[index])} className="border-t border-base-content/10 first:border-t-0">
            <CodeLines lines={diffs[index]?.lines ?? []} />
          </div>
        ))}
    </div>
  );
}

/**
 * The code under the ledger of a parsed file, for each changed function.
 * Closed at first. Hovering a function in the ledger shows its code, and a
 * click pins it open.
 */
export function CodeFooter({
  functions,
  hoveredKey,
  layout = "per_function",
}: {
  functions: ChangedFunction[];
  /** The function being hovered in the ledger. Its code opens while hovered. */
  hoveredKey: string | null;
  layout?: CodeLayout;
}) {
  const loaders = useContext(CodeLoadersContext);
  const changes = useMemo(() => functions.filter((fn) => fn.diff_target !== undefined), [functions]);
  const targets = useMemo(
    () => changes.flatMap((fn) => (fn.diff_target ? [fn.diff_target] : [])),
    [changes],
  );
  const load = useMemo(
    () => (loaders && targets.length > 0 ? () => loaders.loadFunctionDiffs(targets) : null),
    [loaders, targets],
  );
  const loaded = useLoaded(load);

  if (!loaders || changes.length === 0) return null;
  return (
    <FooterFrame>
      <LoadMessage loaded={loaded} />
      {loaded.state === "ready" && layout === "one_block" && (
        <OneBlockCode changes={changes} diffs={loaded.items} hoveredKey={hoveredKey} />
      )}
      {loaded.state === "ready" &&
        layout === "per_function" &&
        changes.map((change, index) => (
          <FunctionCode
            key={functionKey(change)}
            change={change}
            diff={loaded.items[index]}
            hovered={hoveredKey === functionKey(change)}
          />
        ))}
    </FooterFrame>
  );
}

/* -- Other files: the changed lines of the text ------------------------------- */

function FileCode({ diff }: { diff: FileDiff }) {
  const [open, setOpen] = useOpenAllState(false);
  return (
    <div>
      <button
        type="button"
        onClick={() => setOpen((current) => !current)}
        className={SECTION_BUTTON}
      >
        <ChevronOpenIcon open={open} />
        <span className="text-base-content/70">{diff.is_binary ? "Binary file" : "Changed lines"}</span>
        <LineCounts lines={diff.lines} />
      </button>
      {open && diff.is_binary && (
        <p className="px-3 pb-2 font-mono text-[11px] text-base-content/50">
          The contents of a binary file are not shown.
        </p>
      )}
      {open && !diff.is_binary && <CodeLines lines={diff.lines} />}
      {open && diff.omitted_line_count > 0 && (
        <p className="px-3 pb-2 font-mono text-[11px] text-base-content/50">
          {diff.omitted_line_count} more lines are not shown.
        </p>
      )}
    </div>
  );
}

/**
 * The code under the ledger of a file that was not parsed: its changed lines.
 * Closed at first. Click to open it.
 */
export function FileCodeFooter({ file }: { file: ExampleFile }) {
  const loaders = useContext(CodeLoadersContext);
  const target = file.file_diff_target;
  const load = useMemo(
    () => (loaders && target ? () => loaders.loadFileDiffs([target]) : null),
    [loaders, target],
  );
  const loaded = useLoaded(load);

  if (!loaders || !target) return null;
  return (
    <FooterFrame>
      <LoadMessage loaded={loaded} />
      {loaded.state === "ready" && loaded.items[0] && (
        <FileCode diff={loaded.items[0]} />
      )}
    </FooterFrame>
  );
}
