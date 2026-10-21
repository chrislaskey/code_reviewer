import { useState, type FormEvent } from "react";
import { Link } from "react-router-dom";
import { useBackend } from "../../api/context";
import type {
  AnalyzeChangesRequest,
  GetTestCallStackResponse,
  ListTestsResponse,
  TestSummary,
} from "../../api/types";
import { CompareFormView } from "../CompareFormView";
import { buildRequest, saveForm } from "../compare_form";
import { useCompareForm } from "../use_compare_form";
import { CallStackCanvas } from "./CallStackCanvas";
import { CallStackView } from "./CallStackView";
import { TestPicker, testLabel } from "./TestPicker";

/*
 * Pick a comparison and a test, then read the calls below the test top to
 * bottom. Spike: a separate route from the review pages.
 */

type Loaded<T> =
  | { state: "idle" }
  | { state: "loading" }
  | { state: "failed"; message: string }
  | { state: "ready"; value: T };

function shortCommit(commit: string): string {
  return commit.slice(0, 8);
}

export type CallStackRenderer = "html" | "reactflow";

const OTHER_RENDERER_LINK: Record<CallStackRenderer, { to: string; label: string }> = {
  html: { to: "/review/test-call-stack/flow", label: "Open the ReactFlow version" },
  reactflow: { to: "/review/test-call-stack", label: "Open the plain HTML version" },
};

function ErrorLine({ message }: { message: string }) {
  return (
    <p className="border-b border-base-300 bg-rose-500/10 px-4 py-3 font-mono text-xs text-rose-700 dark:text-rose-300">
      {message}
    </p>
  );
}

function Hint({ children }: { children: string }) {
  return <p className="px-6 py-10 text-sm text-base-content/50">{children}</p>;
}

export function TestCallStackScreen({ renderer }: { renderer: CallStackRenderer }) {
  const backend = useBackend();
  const { form, refs, refsError, updateForm } = useCompareForm(backend);
  const [comparison, setComparison] = useState<AnalyzeChangesRequest | null>(null);
  const [tests, setTests] = useState<Loaded<ListTestsResponse>>({ state: "idle" });
  const [selected, setSelected] = useState<TestSummary | null>(null);
  const [stack, setStack] = useState<Loaded<GetTestCallStackResponse>>({ state: "idle" });

  async function listTests(event: FormEvent) {
    event.preventDefault();
    const request = buildRequest(form);
    if (request === null) return;
    // The review form carries highlight and group rules, which do not apply here.
    const { module_rules: _rules, group_flags: _groups, ...changes } = request;
    saveForm(form);
    setComparison(changes);
    setSelected(null);
    setStack({ state: "idle" });
    setTests({ state: "loading" });
    try {
      setTests({ state: "ready", value: await backend.listTests(changes) });
    } catch (e) {
      setTests({ state: "failed", message: e instanceof Error ? e.message : String(e) });
    }
  }

  async function selectTest(test: TestSummary) {
    if (!comparison) return;
    setSelected(test);
    setStack({ state: "loading" });
    try {
      const value = await backend.getTestCallStack({ ...comparison, test: test.function });
      setStack({ state: "ready", value });
    } catch (e) {
      setStack({ state: "failed", message: e instanceof Error ? e.message : String(e) });
    }
  }

  return (
    <div className="flex h-full flex-col">
      <CompareFormView
        form={form}
        refs={refs}
        refsError={refsError}
        loading={tests.state === "loading"}
        onChange={updateForm}
        onSubmit={listTests}
      />
      {tests.state === "failed" && <ErrorLine message={tests.message} />}
      {tests.state === "idle" && (
        <Hint>Enter the path of a git repository with Elixir code, then list its tests.</Hint>
      )}
      {tests.state === "ready" && (
        <>
          <div className="border-b border-base-300 bg-base-100 px-4 py-2 font-mono text-[11px] text-base-content/70">
            {tests.value.tests.length} tests · {shortCommit(tests.value.base_commit)} →{" "}
            {tests.value.head_commit ? shortCommit(tests.value.head_commit) : "working tree"}
            {selected && (
              <>
                {" · "}
                <span className="text-base-content">
                  {selected.function.module} · {testLabel(selected.function)}
                </span>
              </>
            )}
            <Link
              to={OTHER_RENDERER_LINK[renderer].to}
              className="float-right text-primary hover:underline"
            >
              {OTHER_RENDERER_LINK[renderer].label}
            </Link>
          </div>
          <div className="flex min-h-0 flex-1">
            <TestPicker
              tests={tests.value.tests}
              selected={selected?.function ?? null}
              onSelect={selectTest}
            />
            <div className="flex min-w-0 flex-1 flex-col">
              {stack.state === "idle" && <Hint>Pick a test to walk its calls.</Hint>}
              {stack.state === "loading" && <Hint>Walking the calls…</Hint>}
              {stack.state === "failed" && <ErrorLine message={stack.message} />}
              {stack.state === "ready" &&
                (renderer === "reactflow" ? (
                  <CallStackCanvas stack={stack.value} />
                ) : (
                  <CallStackView stack={stack.value} />
                ))}
            </div>
          </div>
        </>
      )}
    </div>
  );
}
