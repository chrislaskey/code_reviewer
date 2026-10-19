import {
  createContext,
  useContext,
  useEffect,
  useState,
  type Dispatch,
  type SetStateAction,
} from "react";

/*
 * "Open all" and "Close all": one command that every ledger and code block
 * follows. `version` counts the commands, so the same command can be sent twice.
 */

export interface OpenAllCommand {
  version: number;
  open: boolean;
}

export const NO_OPEN_ALL_COMMAND: OpenAllCommand = { version: 0, open: true };

export const OpenAllContext = createContext<OpenAllCommand>(NO_OPEN_ALL_COMMAND);

/**
 * Open or closed state that starts as `initial`, then follows each Open all and
 * Close all command. A click on the item still changes it afterwards.
 */
export function useOpenAllState(initial: boolean): [boolean, Dispatch<SetStateAction<boolean>>] {
  const command = useContext(OpenAllContext);
  const [open, setOpen] = useState(initial);
  useEffect(() => {
    if (command.version > 0) setOpen(command.open);
  }, [command.version, command.open]);
  return [open, setOpen];
}

/** The two buttons that send the commands. */
export function OpenAllButtons({ onCommand }: { onCommand: (open: boolean) => void }) {
  const button = "px-3 py-1 bg-base-100 text-base-content/60 hover:text-base-content";
  return (
    <div className="inline-flex overflow-hidden rounded-lg border border-base-300 text-xs">
      <button type="button" onClick={() => onCommand(true)} className={button}>
        Open all
      </button>
      <button
        type="button"
        onClick={() => onCommand(false)}
        className={`${button} border-l border-base-300`}
      >
        Close all
      </button>
    </div>
  );
}
