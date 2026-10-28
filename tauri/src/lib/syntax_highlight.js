// Syntax highlighting for the code of a diff. The colours live in
// ./code.css (`.code-syntax`), one set for each theme, so a theme switch
// needs no rerun.

import hljs from "highlight.js/lib/core";
import bash from "highlight.js/lib/languages/bash";
import css from "highlight.js/lib/languages/css";
import dockerfile from "highlight.js/lib/languages/dockerfile";
import elixir from "highlight.js/lib/languages/elixir";
import go from "highlight.js/lib/languages/go";
import ini from "highlight.js/lib/languages/ini";
import javascript from "highlight.js/lib/languages/javascript";
import json from "highlight.js/lib/languages/json";
import makefile from "highlight.js/lib/languages/makefile";
import markdown from "highlight.js/lib/languages/markdown";
import python from "highlight.js/lib/languages/python";
import ruby from "highlight.js/lib/languages/ruby";
import rust from "highlight.js/lib/languages/rust";
import sql from "highlight.js/lib/languages/sql";
import typescript from "highlight.js/lib/languages/typescript";
import xml from "highlight.js/lib/languages/xml";
import yaml from "highlight.js/lib/languages/yaml";

const LANGUAGES = {
  bash,
  css,
  dockerfile,
  elixir,
  go,
  ini,
  javascript,
  json,
  makefile,
  markdown,
  python,
  ruby,
  rust,
  sql,
  typescript,
  xml,
  yaml,
};

for (const [name, definition] of Object.entries(LANGUAGES)) {
  hljs.registerLanguage(name, definition);
}

const LANGUAGE_BY_EXTENSION = {
  ex: "elixir",
  exs: "elixir",
  heex: "xml",
  eex: "xml",
  leex: "xml",
  html: "xml",
  xml: "xml",
  svg: "xml",
  rs: "rust",
  ts: "typescript",
  tsx: "typescript",
  mts: "typescript",
  cts: "typescript",
  js: "javascript",
  jsx: "javascript",
  mjs: "javascript",
  cjs: "javascript",
  json: "json",
  yml: "yaml",
  yaml: "yaml",
  toml: "ini",
  css: "css",
  md: "markdown",
  sh: "bash",
  bash: "bash",
  zsh: "bash",
  sql: "sql",
  py: "python",
  rb: "ruby",
  go: "go",
};

const LANGUAGE_BY_FILE_NAME = {
  Dockerfile: "dockerfile",
  Makefile: "makefile",
};

/** The language to highlight a file in, or null when it is not known. */
export function languageForPath(path) {
  const fileName = path.split("/").pop() ?? path;
  const byName = LANGUAGE_BY_FILE_NAME[fileName];
  if (byName) return byName;
  const dot = fileName.lastIndexOf(".");
  if (dot < 0) return null;
  return LANGUAGE_BY_EXTENSION[fileName.slice(dot + 1).toLowerCase()] ?? null;
}

const HTML_PIECE = /<span class="([^"]*)">|<\/span>|([^<]+)/g;
const HTML_ENTITIES = {
  "&amp;": "&",
  "&lt;": "<",
  "&gt;": ">",
  "&quot;": '"',
  "&#x27;": "'",
};

function decodeEntities(text) {
  return text.replace(/&(?:amp|lt|gt|quot|#x27);/g, (entity) => HTML_ENTITIES[entity] ?? entity);
}

/**
 * Turns highlight.js output into tokens by line. A token is `{ text, className }`
 * with the highlight.js class, null for plain text. A token that spans lines is
 * cut at each break.
 */
function tokensByLine(html) {
  const lines = [[]];
  const classes = [];
  for (const match of html.matchAll(HTML_PIECE)) {
    const [piece, openedClass, text] = match;
    if (openedClass !== undefined) {
      classes.push(openedClass);
    } else if (piece === "</span>") {
      classes.pop();
    } else if (text !== undefined) {
      const className = classes.length > 0 ? classes[classes.length - 1] : null;
      decodeEntities(text)
        .split("\n")
        .forEach((part, index) => {
          if (index > 0) lines.push([]);
          if (part !== "") lines[lines.length - 1].push({ text: part, className });
        });
    }
  }
  return lines;
}

/** One side of the diff, highlighted as a whole so strings and comments over several lines work. */
function highlightSide(texts, language) {
  if (texts.length === 0) return [];
  try {
    const { value } = hljs.highlight(texts.join("\n"), { language, ignoreIllegals: true });
    const lines = tokensByLine(value);
    return lines.length === texts.length ? lines : null;
  } catch {
    return null;
  }
}

/**
 * The tokens of every line of a diff, in the same order. A line is null when
 * it could not be highlighted. Removed lines are read in the old code, all
 * others in the new code, so each side parses as the code it was.
 */
export function highlightDiffLines(lines, language) {
  if (!language) return lines.map(() => null);

  const oldIndexes = lines.flatMap((line, index) => (line.kind === "added" ? [] : [index]));
  const newIndexes = lines.flatMap((line, index) => (line.kind === "removed" ? [] : [index]));
  const oldTokens = highlightSide(oldIndexes.map((index) => lines[index].text), language);
  const newTokens = highlightSide(newIndexes.map((index) => lines[index].text), language);

  const result = lines.map(() => null);
  const fill = (indexes, tokens, kinds) => {
    if (!tokens) return;
    indexes.forEach((lineIndex, position) => {
      if (kinds.includes(lines[lineIndex].kind)) result[lineIndex] = tokens[position];
    });
  };
  fill(oldIndexes, oldTokens, ["removed"]);
  fill(newIndexes, newTokens, ["context", "added"]);
  return result;
}
