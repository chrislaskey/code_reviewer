// The log both pages show at the bottom: newest first, the last 30.

const MAX_ENTRIES = 30;

let nextId = 0;

export const logEntries = $state([]);

export function log(message, data) {
  logEntries.unshift({ id: nextId++, time: new Date().toLocaleTimeString(), message, data });
  logEntries.splice(MAX_ENTRIES);
}
