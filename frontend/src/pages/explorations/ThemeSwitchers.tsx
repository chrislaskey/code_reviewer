import type { ReactNode } from "react";
import {
  CycleButtonSwitcher,
  DropdownMenuSwitcher,
  SegmentedIconsSwitcher,
  SlidingPillSwitcher,
  TextTabsSwitcher,
  TogglePillSwitcher,
} from "../../theme/ThemeSwitchers";
import { useTheme } from "../../theme/ThemeProvider";
import { CHANGE_STYLES } from "../../explorations/style";
import { ExplorationPage, Option, Section } from "../../explorations/scaffold";

/** A strip like the real top bar, so each switcher is seen where it would live. */
function TopBarMock({ children }: { children: ReactNode }) {
  return (
    <div className="flex items-center gap-5 bg-base-100 px-4 py-2">
      <span className="text-sm font-semibold">Code Reviewer</span>
      <span className="text-xs text-base-content/60">Explorations</span>
      <div className="ml-auto">{children}</div>
    </div>
  );
}

/** Enough colour to judge the theme: text levels, a card, and every status colour. */
function ThemeSample() {
  return (
    <div className="flex flex-wrap items-center gap-4 border-t border-base-300 bg-base-200/40 p-4">
      <div className="w-56 rounded-xl border border-base-300 bg-base-100 p-3 shadow-sm">
        <p className="text-sm font-semibold">Shop.Orders</p>
        <p className="font-mono text-[11px] text-base-content/60">lib/shop/orders.ex</p>
        <ul className="mt-2 space-y-1 font-mono text-xs">
          {(["added", "modified", "removed", "renamed"] as const).map((kind) => (
            <li key={kind} className="flex items-center gap-2">
              <span className={`size-2 rounded-full ${CHANGE_STYLES[kind].dot}`} />
              <span className={CHANGE_STYLES[kind].text}>{CHANGE_STYLES[kind].label}</span>
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}

const OPTIONS = [
  {
    code: "T1",
    title: "Segmented icons",
    note: "System, Light and Dark as three icon buttons, always visible, one click to any choice. The one in the top bar now. Icon-only, so it relies on the tooltip to explain itself.",
    switcher: <SegmentedIconsSwitcher />,
  },
  {
    code: "T2",
    title: "Sliding pill",
    note: "The same three choices in a rounded pill with a thumb that slides to the choice. A little more polish, same footprint.",
    switcher: <SlidingPillSwitcher />,
  },
  {
    code: "T3",
    title: "Cycle button",
    note: "One button showing the current choice by name. Each click moves to the next. Smallest, and the name is explicit, but reaching the third choice takes two clicks.",
    switcher: <CycleButtonSwitcher />,
  },
  {
    code: "T4",
    title: "Light and dark toggle",
    note: "A switch with sun and moon, and no System choice. The simplest idea, but loses following the operating system.",
    switcher: <TogglePillSwitcher />,
  },
  {
    code: "T5",
    title: "Text tabs",
    note: "System, Light, Dark as words with an underline. Clearest wording, a little wider, and looks more like navigation than a setting.",
    switcher: <TextTabsSwitcher />,
  },
  {
    code: "T6",
    title: "Dropdown menu",
    note: "A Theme button that opens a small menu with a check on the current choice. Quietest in the bar, and has room for more options later. Two clicks to change.",
    switcher: <DropdownMenuSwitcher />,
  },
];

export default function ThemeSwitchers() {
  const { preference, resolved } = useTheme();
  return (
    <ExplorationPage
      title="Theme switchers"
      intro={
        <>
          <p>
            Six ways to switch between light and dark. Every one is live: it changes the real
            theme for the whole app, and the choice is remembered.
          </p>
          <p>
            Choice: <span className="font-mono">{preference}</span>, showing{" "}
            <span className="font-mono">{resolved}</span>. System follows the operating system
            and updates when it changes.
          </p>
        </>
      }
    >
      <Section id="switchers" title="Switchers">
        {OPTIONS.map((option) => (
          <Option key={option.code} code={option.code} title={option.title} note={option.note}>
            <TopBarMock>{option.switcher}</TopBarMock>
            <ThemeSample />
          </Option>
        ))}
      </Section>
    </ExplorationPage>
  );
}
