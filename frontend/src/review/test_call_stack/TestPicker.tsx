import { useMemo, useState } from "react";
import type { FunctionId, TestSummary } from "../../api/types";
import { CHANGE_STYLES } from "../../explorations/style";

/*
 * The tests of the head, by file, to pick one. A search box narrows them,
 * and a switch keeps only the ones that look like LiveView tests.
 */

const FIELD_CLASS =
  "w-full rounded-lg border border-base-300 bg-base-100 px-2.5 py-1.5 font-mono text-xs text-base-content placeholder:text-base-content/30";

export function sameFunction(a: FunctionId, b: FunctionId): boolean {
  return a.module === b.module && a.name === b.name && a.arity === b.arity;
}

/** The description of the test, without the `test ` the backend puts in front. */
export function testLabel(test: FunctionId): string {
  return test.name.replace(/^test /, "");
}

function groupByFile(tests: TestSummary[]): { filePath: string; tests: TestSummary[] }[] {
  const groups = new Map<string, TestSummary[]>();
  for (const test of tests) {
    const group = groups.get(test.file_path);
    if (group) group.push(test);
    else groups.set(test.file_path, [test]);
  }
  return [...groups].map(([filePath, tests]) => ({ filePath, tests }));
}

export function TestPicker({
  tests,
  selected,
  onSelect,
}: {
  tests: TestSummary[];
  selected: FunctionId | null;
  onSelect: (test: TestSummary) => void;
}) {
  const hasLiveViewTests = tests.some((test) => test.is_live_view_test);
  const [liveViewOnly, setLiveViewOnly] = useState(hasLiveViewTests);
  const [search, setSearch] = useState("");

  const shown = useMemo(() => {
    const needle = search.trim().toLowerCase();
    return tests.filter(
      (test) =>
        (!liveViewOnly || test.is_live_view_test) &&
        (needle === "" ||
          `${test.file_path} ${test.function.module} ${test.function.name}`
            .toLowerCase()
            .includes(needle)),
    );
  }, [tests, liveViewOnly, search]);
  const groups = useMemo(() => groupByFile(shown), [shown]);

  return (
    <aside className="flex w-80 shrink-0 flex-col border-r border-base-300 bg-base-100">
      <div className="space-y-2 border-b border-base-300 p-3">
        <input
          value={search}
          placeholder="Find a test"
          onChange={(event) => setSearch(event.target.value)}
          spellCheck={false}
          className={FIELD_CLASS}
        />
        <label className="flex items-center gap-2 text-[11px] text-base-content/60">
          <input
            type="checkbox"
            checked={liveViewOnly}
            onChange={(event) => setLiveViewOnly(event.target.checked)}
          />
          LiveView tests only
        </label>
        <p className="font-mono text-[11px] text-base-content/50">
          {shown.length} of {tests.length} tests
        </p>
      </div>
      <div className="min-h-0 flex-1 overflow-auto">
        {groups.map((group) => (
          <div key={group.filePath} className="border-b border-base-300 py-1">
            <p
              className="truncate px-3 py-1 font-mono text-[10px] text-base-content/50"
              style={{ direction: "rtl" }}
              title={group.filePath}
            >
              {group.filePath}
            </p>
            {group.tests.map((test) => {
              const isSelected = selected !== null && sameFunction(selected, test.function);
              const change = test.change ? CHANGE_STYLES[test.change] : null;
              return (
                <button
                  key={`${test.function.module}:${test.function.name}`}
                  type="button"
                  onClick={() => onSelect(test)}
                  className={`flex w-full items-center gap-2 px-3 py-1 text-left font-mono text-[11px] ${
                    isSelected
                      ? "bg-base-content/10 text-base-content"
                      : "text-base-content/80 hover:bg-base-content/5"
                  }`}
                >
                  <span
                    className={`size-1.5 shrink-0 rounded-full ${change ? change.dot : "bg-base-content/20"}`}
                    title={change ? `test ${change.label}` : "test unchanged"}
                  />
                  <span className="truncate">{testLabel(test.function)}</span>
                </button>
              );
            })}
          </div>
        ))}
        {groups.length === 0 && (
          <p className="px-3 py-4 text-[11px] text-base-content/50">No tests match.</p>
        )}
      </div>
    </aside>
  );
}
