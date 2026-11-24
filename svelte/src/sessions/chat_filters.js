// Which messages of an agent's chat a session's page shows.
import { t } from "ttag";

/** The filters to pick from, each labelled and explained. */
export function chatFilters() {
  return [
    { value: "all", label: t`All`, explanation: t`Every message, with the notes the agent's CLI writes.` },
    { value: "last", label: t`Last`, explanation: t`Each of your prompts with the agent's final reply to it.` },
    { value: "yours", label: t`Yours`, explanation: t`Only the prompts you wrote.` },
    { value: "none", label: t`None`, explanation: t`No messages, to bring the logs below into view.` },
  ];
}

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
