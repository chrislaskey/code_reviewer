import { useState } from "react";
import {
  CheckIcon,
  ChevronDownIcon,
  ComputerDesktopIcon,
  MoonIcon,
  SunIcon,
} from "@heroicons/react/16/solid";
import type { IconComponent } from "../components/card";
import { useTheme, type ThemePreference } from "./ThemeProvider";

/*
 * Every way the theme can be switched. All of them read and write the same
 * theme, so any of them can sit in the top bar. `ThemeSwitcher` is the one
 * the app uses.
 */

interface ThemeOption {
  value: ThemePreference;
  label: string;
  icon: IconComponent;
}

export const THEME_OPTIONS: ThemeOption[] = [
  { value: "system", label: "System", icon: ComputerDesktopIcon },
  { value: "light", label: "Light", icon: SunIcon },
  { value: "dark", label: "Dark", icon: MoonIcon },
];

const LIGHT_DARK_OPTIONS = THEME_OPTIONS.filter((option) => option.value !== "system");

function optionFor(value: ThemePreference): ThemeOption {
  return THEME_OPTIONS.find((option) => option.value === value) ?? THEME_OPTIONS[0];
}

/* -- Segmented icons: system, light, dark, always visible ------------------- */

export function SegmentedIconsSwitcher() {
  const { preference, setPreference } = useTheme();
  return (
    <div
      role="radiogroup"
      aria-label="Theme"
      className="inline-flex overflow-hidden rounded-lg border border-base-300 bg-base-100"
    >
      {THEME_OPTIONS.map(({ value, label, icon: Icon }) => {
        const selected = value === preference;
        return (
          <button
            key={value}
            type="button"
            role="radio"
            aria-checked={selected}
            aria-label={label}
            title={label}
            onClick={() => setPreference(value)}
            className={[
              "flex size-7 items-center justify-center transition-colors",
              selected
                ? "bg-base-content text-base-100"
                : "text-base-content/50 hover:text-base-content",
            ].join(" ")}
          >
            <Icon className="size-4" />
          </button>
        );
      })}
    </div>
  );
}

/* -- Sliding pill: the same three choices, with a thumb that moves ---------- */

export function SlidingPillSwitcher() {
  const { preference, setPreference } = useTheme();
  const index = THEME_OPTIONS.findIndex((option) => option.value === preference);
  return (
    <div
      role="radiogroup"
      aria-label="Theme"
      className="relative inline-flex rounded-full border border-base-300 bg-base-200 p-0.5"
    >
      <span
        aria-hidden="true"
        style={{ transform: `translateX(${index * 100}%)` }}
        className="absolute left-0.5 top-0.5 size-7 rounded-full bg-base-100 shadow-sm ring-1 ring-base-300 transition-transform duration-200"
      />
      {THEME_OPTIONS.map(({ value, label, icon: Icon }) => (
        <button
          key={value}
          type="button"
          role="radio"
          aria-checked={value === preference}
          aria-label={label}
          title={label}
          onClick={() => setPreference(value)}
          className={[
            "relative flex size-7 items-center justify-center rounded-full transition-colors",
            value === preference ? "text-base-content" : "text-base-content/40 hover:text-base-content",
          ].join(" ")}
        >
          <Icon className="size-4" />
        </button>
      ))}
    </div>
  );
}

/* -- Cycle button: one icon, each click goes to the next choice ------------- */

export function CycleButtonSwitcher() {
  const { preference, setPreference } = useTheme();
  const current = optionFor(preference);
  const next = THEME_OPTIONS[(THEME_OPTIONS.indexOf(current) + 1) % THEME_OPTIONS.length];
  const Icon = current.icon;
  return (
    <button
      type="button"
      onClick={() => setPreference(next.value)}
      title={`Theme: ${current.label}. Click for ${next.label}.`}
      className="flex items-center gap-1.5 rounded-lg border border-base-300 bg-base-100 px-2.5 py-1 text-xs text-base-content/70 transition-colors hover:text-base-content"
    >
      <Icon className="size-4" />
      {current.label}
    </button>
  );
}

/* -- Toggle: light or dark only, no system choice --------------------------- */

export function TogglePillSwitcher() {
  const { resolved, setPreference } = useTheme();
  const dark = resolved === "dark";
  const [Light, Dark] = [LIGHT_DARK_OPTIONS[0].icon, LIGHT_DARK_OPTIONS[1].icon];
  return (
    <button
      type="button"
      role="switch"
      aria-checked={dark}
      aria-label="Dark theme"
      onClick={() => setPreference(dark ? "light" : "dark")}
      className={[
        "relative inline-flex h-7 w-14 items-center rounded-full border border-base-300 px-1 transition-colors",
        dark ? "bg-base-content/80" : "bg-base-200",
      ].join(" ")}
    >
      <Light className="size-4 text-amber-500" />
      <Dark className="ml-auto size-4 text-sky-300" />
      <span
        aria-hidden="true"
        className={[
          "absolute top-0.5 size-5 rounded-full bg-base-100 shadow transition-all duration-200",
          dark ? "left-[calc(100%-1.375rem)]" : "left-0.5",
        ].join(" ")}
      />
    </button>
  );
}

/* -- Text tabs: words, no icons --------------------------------------------- */

export function TextTabsSwitcher() {
  const { preference, setPreference } = useTheme();
  return (
    <div role="radiogroup" aria-label="Theme" className="inline-flex gap-3 text-xs">
      {THEME_OPTIONS.map(({ value, label }) => (
        <button
          key={value}
          type="button"
          role="radio"
          aria-checked={value === preference}
          onClick={() => setPreference(value)}
          className={[
            "border-b-2 pb-0.5 transition-colors",
            value === preference
              ? "border-base-content font-semibold text-base-content"
              : "border-transparent text-base-content/50 hover:text-base-content",
          ].join(" ")}
        >
          {label}
        </button>
      ))}
    </div>
  );
}

/* -- Dropdown: one button, a small menu ------------------------------------- */

export function DropdownMenuSwitcher() {
  const { preference, setPreference } = useTheme();
  const [open, setOpen] = useState(false);
  const current = optionFor(preference);
  const CurrentIcon = current.icon;

  return (
    <div className="relative inline-block">
      <button
        type="button"
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
        className="flex items-center gap-1.5 rounded-lg border border-base-300 bg-base-100 px-2.5 py-1 text-xs text-base-content/70 transition-colors hover:text-base-content"
      >
        <CurrentIcon className="size-4" />
        Theme
        <ChevronDownIcon className="size-3.5" />
      </button>
      {open && (
        <>
          <button
            type="button"
            aria-label="Close theme menu"
            onClick={() => setOpen(false)}
            className="fixed inset-0 z-10 cursor-default"
          />
          <ul
            role="menu"
            className="absolute right-0 z-20 mt-1 w-36 overflow-hidden rounded-lg border border-base-300 bg-base-100 py-1 text-xs shadow-lg"
          >
            {THEME_OPTIONS.map(({ value, label, icon: Icon }) => (
              <li key={value}>
                <button
                  type="button"
                  role="menuitemradio"
                  aria-checked={value === preference}
                  onClick={() => {
                    setPreference(value);
                    setOpen(false);
                  }}
                  className="flex w-full items-center gap-2 px-3 py-1.5 text-left transition-colors hover:bg-base-content/5"
                >
                  <Icon className="size-4 text-base-content/60" />
                  <span className="flex-1">{label}</span>
                  {value === preference && <CheckIcon className="size-3.5" />}
                </button>
              </li>
            ))}
          </ul>
        </>
      )}
    </div>
  );
}

/** The switcher the app shows in its top bar. */
export const ThemeSwitcher = SegmentedIconsSwitcher;
