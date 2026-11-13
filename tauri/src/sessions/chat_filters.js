// Which messages of an agent's chat a session's page shows.

export const CHAT_FILTERS = [
  { value: "all", label: "All" },
  { value: "last", label: "Last" },
  { value: "yours", label: "Yours" },
  { value: "none", label: "None" },
];

/** A message the user typed, not only tool results passed back to the agent. */
function isPrompt(message) {
  return message.role === "user" && message.parts.some((part) => part.type === "text");
}

/**
 * The messages a filter keeps: `all`, with the notes the agent's CLI writes; `last`, each prompt with the agent's
 * last message before the next prompt (its final reply); `yours`, the prompts; `none`, nothing, to bring the logs
 * below into view.
 */
export function filterChatMessages(messages, filter) {
  if (filter === "none") return [];
  if (filter === "yours") return messages.filter(isPrompt);
  if (filter !== "last") return messages;
  const chat = messages.filter((message) => message.role !== "note");
  return chat.filter((message, index) => {
    if (isPrompt(message)) return true;
    const next = chat[index + 1];
    return message.role === "assistant" && (next === undefined || isPrompt(next));
  });
}
