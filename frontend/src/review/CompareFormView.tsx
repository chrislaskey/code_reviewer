import type { FormEvent, ReactNode } from "react";
import type { ListBranchesAndRecentCommitsResponse } from "../api/types";
import { SegmentedControl } from "../explorations/controls";
import {
  COMPARE_MODE_OPTIONS,
  buildRequest,
  describeComparison,
  type CompareForm,
} from "./compare_form";

const FIELD_CLASS =
  "rounded-lg border border-base-300 bg-base-100 px-2.5 py-1.5 font-mono text-xs text-base-content placeholder:text-base-content/30";

function Field({
  label,
  className,
  children,
}: {
  label: string;
  className?: string;
  children: ReactNode;
}) {
  return (
    <label className={`flex flex-col gap-1 text-[11px] text-base-content/60 ${className ?? ""}`}>
      {label}
      {children}
    </label>
  );
}

function SelectField({
  label,
  value,
  options,
  className,
  onChange,
}: {
  label: string;
  value: string;
  options: { value: string; label: string }[];
  className?: string;
  onChange: (value: string) => void;
}) {
  // Until the repository has been read, show the remembered choice.
  const shown =
    value !== "" && !options.some((option) => option.value === value)
      ? [{ value, label: /^[0-9a-f]{40}$/.test(value) ? value.slice(0, 8) : value }, ...options]
      : options;
  return (
    <Field label={label} className={className}>
      <select
        value={value}
        onChange={(event) => onChange(event.target.value)}
        className={FIELD_CLASS}
      >
        {shown.length === 0 && <option value="">None found</option>}
        {shown.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
    </Field>
  );
}

function RulesField({
  label,
  placeholder,
  value,
  onChange,
}: {
  label: string;
  placeholder: string;
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <Field label={label} className="min-w-56 flex-1">
      <textarea
        value={value}
        placeholder={placeholder}
        onChange={(event) => onChange(event.target.value)}
        rows={3}
        spellCheck={false}
        className={FIELD_CLASS}
      />
    </Field>
  );
}

/** Pick the repository, what to compare, and which modules to highlight or ignore. */
export function CompareFormView({
  form,
  refs,
  refsError,
  loading,
  onChange,
  onSubmit,
}: {
  form: CompareForm;
  refs: ListBranchesAndRecentCommitsResponse | null;
  refsError: string | null;
  loading: boolean;
  onChange: (changes: Partial<CompareForm>) => void;
  onSubmit: (event: FormEvent) => void;
}) {
  const branchOptions = (refs?.branches ?? []).map((branch) => ({ value: branch, label: branch }));
  const commitOptions = (refs?.recent_commits ?? []).map((commit) => ({
    value: commit.sha,
    label: `${commit.short_sha}  ${commit.subject}  (${commit.committed_on})`,
  }));
  const hasRules = form.includeRules.trim() !== "" || form.ignoreRules.trim() !== "";

  return (
    <form
      onSubmit={onSubmit}
      className="space-y-3 border-b border-base-300 bg-base-100 px-4 py-3"
    >
      <div className="flex flex-wrap items-end gap-3">
        <Field label="Repository path" className="min-w-72 flex-1">
          <input
            value={form.repoPath}
            placeholder="/path/to/elixir/project"
            onChange={(event) => onChange({ repoPath: event.target.value })}
            spellCheck={false}
            className={FIELD_CLASS}
          />
        </Field>
        <SegmentedControl
          value={form.mode}
          onChange={(mode) => onChange({ mode })}
          options={COMPARE_MODE_OPTIONS}
        />
      </div>

      {form.mode === "commit" && (
        <SelectField
          label="Commit"
          value={form.commitSha}
          options={commitOptions}
          onChange={(commitSha) => onChange({ commitSha })}
        />
      )}
      {form.mode === "branch" && (
        <div className="flex flex-wrap items-end gap-3">
          <SelectField
            label="Review this branch"
            value={form.headBranch}
            options={branchOptions}
            className="min-w-48"
            onChange={(headBranch) => onChange({ headBranch })}
          />
          <SelectField
            label="Against"
            value={form.baseBranch}
            options={branchOptions}
            className="min-w-48"
            onChange={(baseBranch) => onChange({ baseBranch })}
          />
        </div>
      )}

      <details open={hasRules}>
        <summary className="cursor-pointer text-[11px] text-base-content/60">
          Highlight or ignore modules
        </summary>
        <div className="mt-2 space-y-2">
          <div className="flex flex-wrap gap-3">
            <RulesField
              label="Also highlight (one per line)"
              placeholder={"Ecto.Repo\nShopWeb"}
              value={form.includeRules}
              onChange={(includeRules) => onChange({ includeRules })}
            />
            <RulesField
              label="Ignore (one per line)"
              placeholder={"Enum.reduce\nShopTest*"}
              value={form.ignoreRules}
              onChange={(ignoreRules) => onChange({ ignoreRules })}
            />
          </div>
          <p className="text-[11px] text-base-content/50">
            Calls to modules the project defines are always highlighted. A line is a module
            (<code>Shop</code> covers <code>Shop.Orders</code> but not <code>ShopWeb</code>), a
            function (<code>Enum.reduce</code> or <code>Enum.reduce/3</code>), or a prefix ending
            in <code>*</code> (<code>Shop*</code> covers <code>ShopWeb</code> too). Ignored
            callers and calls disappear; ignoring beats highlighting.
          </p>
        </div>
      </details>

      <div className="flex flex-wrap items-center gap-3">
        <button
          type="submit"
          disabled={loading || buildRequest(form) === null}
          className="rounded-lg bg-blue-600 px-4 py-1.5 text-xs font-medium text-white hover:bg-blue-700 disabled:opacity-50"
        >
          {loading ? "Analyzing…" : "Analyze changes"}
        </button>
        <p className="text-[11px] text-base-content/50">{describeComparison(form)}</p>
        {refsError && form.mode !== "working_tree" && (
          <p className="text-[11px] text-rose-600 dark:text-rose-400">
            Could not read the branches: {refsError}
          </p>
        )}
      </div>
    </form>
  );
}
