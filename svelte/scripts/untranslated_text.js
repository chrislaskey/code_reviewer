// The English in the pages' markup that skips ttag, for
// translations_check.js: text between tags, and the attributes a person
// reads or hears (TEXT_ATTRIBUTES), written as plain strings rather than
// `{t`...`}`, and the plain strings an expression there shows, such as
// `{busy ? "Saving" : "Save"}`. Strings in <script> are not checked. Read
// with Svelte's own parser. A comment that starts with "untranslated"
// exempts the node after it, for a name or a key's label:
// `<!-- untranslated: the app's name --><Heading>Starter Pack</Heading>`.
import { readFileSync, readdirSync } from "node:fs";
import { join, relative } from "node:path";
import { parse } from "svelte/compiler";

const TEXT_ATTRIBUTES = new Set([
  "alt",
  "aria-description",
  "aria-label",
  "label",
  "missing",
  "placeholder",
  "submitText",
  "text",
  "title",
]);
const HAS_LETTERS = /\p{L}/u;
// Parts of the tree that hold expressions, never markup
const EXPRESSION_KEYS = new Set(["attributes", "context", "expression", "key", "metadata", "name_loc", "test"]);

/**
 * Every place in `folder`'s .svelte files, outside `ignored` folders, as
 * `path:line: "text"`.
 * @param {string} root the folder paths are shown from
 * @param {string} folder
 * @param {string[]} ignored folders, from `root`
 */
export function untranslatedText(root, folder, ignored) {
  return svelteFiles(root, folder, ignored).flatMap((path) => fileProblems(root, path));
}

/**
 * @param {string} root
 * @param {string} folder
 * @param {string[]} ignored
 * @returns {string[]}
 */
function svelteFiles(root, folder, ignored) {
  return readdirSync(folder, { withFileTypes: true }).flatMap((entry) => {
    const path = join(folder, entry.name);
    if (entry.isDirectory()) return ignored.includes(relative(root, path)) ? [] : svelteFiles(root, path, ignored);
    return entry.name.endsWith(".svelte") ? [path] : [];
  });
}

/**
 * @param {string} root
 * @param {string} path
 */
function fileProblems(root, path) {
  const source = readFileSync(path, "utf8");
  const problems = [];
  /** @param {{start: number}} node @param {string} text */
  const found = (node, text) => {
    const line = source.slice(0, node.start).split("\n").length;
    problems.push(`${relative(root, path)}:${line}: ${JSON.stringify(text.trim().replace(/\s+/g, " "))}`);
  };
  fragmentsWalk(parse(source, { modern: true }).fragment, found);
  return problems;
}

/**
 * Checks each node of every fragment in `tree`.
 * @param {any} tree
 * @param {(node: any, text: string) => void} found
 */
function fragmentsWalk(tree, found) {
  if (tree?.type === "Fragment") {
    let exempt = false;
    for (const node of tree.nodes) {
      const isBlank = node.type === "Text" && !node.data.trim();
      if (isBlank) continue;
      if (node.type === "Comment") {
        exempt = node.data.trim().startsWith("untranslated");
        continue;
      }
      if (!exempt) nodeCheck(node, found);
      if (!exempt) fragmentsWalk(node, found);
      exempt = false;
    }
    return;
  }
  for (const [key, value] of Object.entries(tree ?? {})) {
    if (!EXPRESSION_KEYS.has(key) && value && typeof value === "object") fragmentsWalk(value, found);
  }
}

/**
 * @param {any} node
 * @param {(node: any, text: string) => void} found
 */
function nodeCheck(node, found) {
  if (node.type === "Text" && HAS_LETTERS.test(node.data)) found(node, node.data);
  if (node.type === "ExpressionTag") {
    for (const text of stringsShown(node.expression)) found(node, text);
  }
  for (const attribute of node.attributes ?? []) {
    if (attribute.type !== "Attribute" || !TEXT_ATTRIBUTES.has(attribute.name)) continue;
    const parts = Array.isArray(attribute.value) ? attribute.value : [attribute.value];
    for (const part of parts) {
      if (part?.type === "Text" && HAS_LETTERS.test(part.data)) found(attribute, `${attribute.name}="${part.data}"`);
      if (part?.type === "ExpressionTag") {
        for (const text of stringsShown(part.expression)) found(attribute, `${attribute.name}={${JSON.stringify(text)}}`);
      }
    }
  }
}

/**
 * The plain strings with letters an expression can evaluate to: itself,
 * either branch of a `? :`, either side of `||` or `??`; never a string
 * it only compares or passes on, as in `status === "waiting"`.
 * @param {any} expression
 * @returns {string[]}
 */
function stringsShown(expression) {
  switch (expression?.type) {
    case "Literal":
      return typeof expression.value === "string" && HAS_LETTERS.test(expression.value) ? [expression.value] : [];
    case "TemplateLiteral": {
      const text = expression.quasis.map((quasi) => quasi.value.cooked).join("…");
      return HAS_LETTERS.test(text) ? [text] : [];
    }
    case "ConditionalExpression":
      return [...stringsShown(expression.consequent), ...stringsShown(expression.alternate)];
    case "LogicalExpression":
      return [...stringsShown(expression.left), ...stringsShown(expression.right)];
    default:
      return [];
  }
}
