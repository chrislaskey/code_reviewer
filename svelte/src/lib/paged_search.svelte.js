// A search the core answers a page at a time, `{total, files}`, as the
// files index and the quick picker show it: only the pages of the
// positions shown are asked for, a hole where a page has not come yet.
// Starting over asks again for the pages on screen and replaces them at
// once; an answer to an older start is dropped. See
// docs/design/files-browsing.md.

export const PAGE_SIZE = 200;

export class PagedSearch {
  /** @type {number | null} how many match, null until the first answer */
  total = $state(null);
  /** @type {any[]} the files by position, a hole where one has not come yet */
  rows = $state.raw([]);
  error = $state("");

  /** @type {(offset: number, limit: number) => Promise<{total: number, files: any[]}>} */
  #searchPage;
  /** The pages asked for since the search started over. */
  #pagesAsked = new Set();
  /** Counts the times the search started over, so an older answer is dropped. */
  #generation = 0;
  /** The positions on screen, which starting over fetches again. */
  #shown = { start: 0, end: PAGE_SIZE };

  /** @param {(offset: number, limit: number) => Promise<{total: number, files: any[]}>} searchPage */
  constructor(searchPage) {
    this.#searchPage = searchPage;
  }

  /** Asks again for every page on screen, and shows the answers together. */
  async startOver() {
    this.#generation += 1;
    const asked = this.#generation;
    const pages = pagesOf(this.#shown.start, this.#shown.end);
    this.#pagesAsked = new Set(pages);
    try {
      const answers = await Promise.all(pages.map((page) => this.#searchPage(page * PAGE_SIZE, PAGE_SIZE)));
      if (asked !== this.#generation) return;
      const next = [];
      answers.forEach(({ files }, index) => place(next, pages[index], files));
      this.total = answers[answers.length - 1].total;
      this.rows = next;
      this.error = "";
    } catch (problem) {
      if (asked === this.#generation) this.error = String(problem);
    }
  }

  /** Starts over from the first page, as a new search does. */
  startOverFromTop() {
    this.#shown = { start: 0, end: PAGE_SIZE };
    return this.startOver();
  }

  /**
   * Fetches the pages of the positions shown that have not been asked for.
   * @param {number} start
   * @param {number} end
   */
  showPositions(start, end) {
    this.#shown = { start, end };
    for (const page of pagesOf(start, end)) this.#loadPage(page);
  }

  /** @param {number} page */
  async #loadPage(page) {
    if (this.#pagesAsked.has(page)) return;
    this.#pagesAsked.add(page);
    const asked = this.#generation;
    try {
      const { total, files } = await this.#searchPage(page * PAGE_SIZE, PAGE_SIZE);
      if (asked !== this.#generation) return;
      const next = this.rows.slice();
      place(next, page, files);
      this.total = total;
      this.rows = next;
    } catch (problem) {
      if (asked !== this.#generation) return;
      this.#pagesAsked.delete(page);
      this.error = String(problem);
    }
  }
}

/**
 * @param {any[]} files the files by position, changed in place
 * @param {number} page
 * @param {any[]} found
 */
function place(files, page, found) {
  found.forEach((file, index) => (files[page * PAGE_SIZE + index] = file));
}

/**
 * @param {number} start
 * @param {number} end
 */
function pagesOf(start, end) {
  const pages = [];
  for (let page = Math.floor(start / PAGE_SIZE); page * PAGE_SIZE < Math.max(end, 1); page += 1) pages.push(page);
  return pages;
}
