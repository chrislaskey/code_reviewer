// Which messages of an agent's chat a session's page shows.

export const CHAT_FILTERS = [
  { value: "all", label: "All" },
  { value: "last_only", label: "Last only" },
  { value: "yours_only", label: "Yours only" },
];

/** A message the user typed, not only tool results passed back to the agent. */
function isPrompt(message) {
  return message.role === "user" && message.parts.some((part) => part.type === "text");
}

/**
 * The messages a filter keeps: `all`; `last_only`, each prompt with the agent's last message before the next
 * prompt (its final reply); `yours_only`, the prompts.
 */
export function filterChatMessages(messages, filter) {
  if (filter === "yours_only") return messages.filter(isPrompt);
  if (filter !== "last_only") return messages;
  return messages.filter((message, index) => {
    if (isPrompt(message)) return true;
    const next = messages[index + 1];
    return message.role === "assistant" && (next === undefined || isPrompt(next));
  });
}
