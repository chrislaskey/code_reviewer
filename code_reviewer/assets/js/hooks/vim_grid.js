// VimGrid: keyboard cursor over a <table> inside the hook element, with
// fold-style expand/collapse of module rows and of diffs.
//
// Rows are module rows (data-kind="module"), function rows that point at
// their module (data-parent), or diff rows (data-kind="diff") streamed in
// under a function or module by the server. The cursor moves over "stops":
// every visible module and function row, plus every line (data-line) of
// every visible diff, in display order. An open diff is an open fold, so `j`
// on a function walks into its diff and out the other side. Collapsed
// modules hide their function rows and those functions' diffs; the cursor
// only ever visits visible stops. Movement and module folds are client side
// so they stay instant; Enter and the diff folds talk to the server. The
// server re-renders freely: `updated()` re-applies cursor and fold state.
//
//   h j k l / arrows   move by one (prefix a count: 5j)
//   gg / G             first / last stop (3G = stop 3)
//   0 ^ / $            first / last column
//   Ctrl-d / Ctrl-u    half page down / up
//   { / }              previous / next table row, skipping diff lines
//   n / N              next / previous change (a run of +/− lines)
//   Enter              on a module row: toggle its functions
//                      on a module row's +/− cell: toggle the whole-module diff
//                      on a function row: toggle its diff
//                      on a diff line: "activate" that line on the server
//   za / zo / zc       toggle / open / close the fold under the cursor:
//                      a function row or diff line folds the diff, a module
//                      row folds the module. zc on a function whose diff is
//                      closed closes its module, as in vim.
//   zR / zM            open / close every module
//   Escape             clear pending keys; on a diff line, back to its row

const CURSOR = "is-cursor"
const CURSOR_ROW = "is-cursor-row"
const COLLAPSED = "is-collapsed"

const VimGrid = {
  mounted() {
    this.stopId = null
    this.lastIdx = 0
    this.col = 0
    this.count = ""
    this.pending = "" // "g" or "z" awaiting a second key
    // Module view by default: every module starts collapsed. `seen` lets
    // updated() collapse only modules that arrive later.
    this.seen = new Set(this.moduleRows().map((tr) => tr.dataset.id))
    this.collapsed = new Set(this.seen)

    this.onKeydown = (e) => this.handleKey(e)
    window.addEventListener("keydown", this.onKeydown)

    this.el.addEventListener("click", (e) => {
      // nearest first: a diff line before the cell that holds it
      const hit = e.target.closest("tbody [data-line], tbody td")
      if (!hit || !this.el.contains(hit)) return
      let stop = hit
      if (!this.isLine(hit)) {
        const tr = hit.parentElement
        if (!tr.dataset.id || tr.dataset.kind === "diff") return
        stop = tr
        this.col = hit.cellIndex
      }
      this.stopId = stop.dataset.id
      // a click is Enter on that stop
      this.activate(stop)
      this.render()
    })

    this.render()
  },

  updated() {
    // New data may bring new modules; they start collapsed too.
    const known = new Set(this.moduleRows().map((tr) => tr.dataset.id))
    for (const id of known) if (!this.seen.has(id)) this.collapsed.add(id)
    this.seen = known
    this.render()
  },

  destroyed() {
    window.removeEventListener("keydown", this.onKeydown)
  },

  // -- stops -------------------------------------------------------------------

  allRows() {
    return Array.from(this.el.querySelectorAll("tbody tr[data-id]"))
  },

  moduleRows() {
    return this.allRows().filter((tr) => tr.dataset.kind === "module")
  },

  isLine(el) {
    return !!el && "line" in el.dataset
  },

  // The <tr> a stop lives in: itself, or the diff row around a line.
  rowOf(el) {
    return this.isLine(el) ? el.closest("tr") : el
  },

  // The module whose fold hides this row, if any. Function rows fold with
  // their module; so do function diffs (data-fold). Module diffs never fold.
  foldOwner(tr) {
    return (tr.dataset.kind === "diff" ? tr.dataset.fold : tr.dataset.parent) || null
  },

  folded(tr) {
    const owner = this.foldOwner(tr)
    return !!owner && this.collapsed.has(owner)
  },

  // Cursor stops in display order: visible module and function rows, and
  // the lines of visible diffs.
  stops() {
    const out = []
    for (const tr of this.allRows()) {
      if (this.folded(tr)) continue
      if (tr.dataset.kind === "diff") out.push(...tr.querySelectorAll("[data-line]"))
      else out.push(tr)
    }
    return out
  },

  stopById(id) {
    return id ? this.el.querySelector(`tbody [data-id="${CSS.escape(id)}"]`) : null
  },

  columnCount() {
    const first = this.allRows().find((tr) => tr.dataset.kind !== "diff")
    return first ? first.children.length : 0
  },

  currentIndex(stops) {
    const idx = stops.findIndex((el) => el.dataset.id === this.stopId)
    // A stop that vanished (a diff closed under the cursor, a reload) leaves
    // the cursor where it was rather than at the top.
    return idx === -1 ? Math.min(this.lastIdx, Math.max(stops.length - 1, 0)) : idx
  },

  indexOfId(stops, id, fallback) {
    const idx = stops.findIndex((el) => el.dataset.id === id)
    return idx === -1 ? fallback : idx
  },

  pageSize() {
    const tr = this.allRows().find((row) => row.dataset.kind !== "diff")
    const rowHeight = tr ? tr.getBoundingClientRect().height || 28 : 28
    return Math.max(1, Math.floor(window.innerHeight / rowHeight / 2))
  },

  // -- motions ---------------------------------------------------------------------

  // Index of the n-th stop from `idx` passing `test`, in the direction of
  // n's sign. Stops at the last match when there are fewer than n.
  nth(stops, idx, n, test) {
    const step = n < 0 ? -1 : 1
    let i = idx
    for (let left = Math.abs(n); left > 0; left--) {
      let j = i + step
      while (j >= 0 && j < stops.length && !test(stops[j], j)) j += step
      if (j < 0 || j >= stops.length) break
      i = j
    }
    return i
  },

  // { and }: table rows only, so a long diff can be skipped in one key
  nthRow(stops, idx, n) {
    return this.nth(stops, idx, n, (el) => !this.isLine(el))
  },

  // n and N: the first line of each run of +/− lines
  nthChange(stops, idx, n) {
    return this.nth(stops, idx, n, (el, i) => this.isChange(el) && !this.isChange(stops[i - 1]))
  },

  isChange(el) {
    return this.isLine(el) && (el.dataset.kind === "add" || el.dataset.kind === "del")
  },

  // -- folds -------------------------------------------------------------------

  moduleIdFor(el) {
    const tr = this.rowOf(el)
    if (!tr) return null
    switch (tr.dataset.kind) {
      case "module": return tr.dataset.id
      case "diff": return tr.dataset.fold || tr.dataset.parent
      default: return tr.dataset.parent
    }
  },

  toggle(moduleId) {
    if (!moduleId) return
    if (this.collapsed.has(moduleId)) this.collapsed.delete(moduleId)
    else this.collapse(moduleId)
  },

  collapse(moduleId) {
    if (!moduleId) return
    this.collapsed.add(moduleId)
    // a cursor inside the fold lands on the module row
    const tr = this.rowOf(this.stopById(this.stopId))
    if (tr && this.foldOwner(tr) === moduleId) this.stopId = moduleId
  },

  open(moduleId) {
    if (moduleId) this.collapsed.delete(moduleId)
  },

  // Diffs are folds too, kept on the server. The cursor goes to the row
  // that owns the diff, which is where it already is unless it was inside.
  diffOpen(ownerId) {
    return !!this.el.querySelector(`tbody tr[data-kind="diff"][data-parent="${CSS.escape(ownerId)}"]`)
  },

  toggleDiff(ownerId) {
    if (!ownerId) return
    this.stopId = ownerId
    this.pushEvent("toggle_diff", {id: ownerId})
  },

  // za
  foldToggle(el) {
    const tr = this.rowOf(el)
    if (!tr) return
    if (tr.dataset.kind === "module") return this.toggle(tr.dataset.id)
    if (tr.dataset.kind === "diff") return this.toggleDiff(tr.dataset.parent)
    this.toggleDiff(tr.dataset.id)
  },

  // zo
  foldOpen(el) {
    const tr = this.rowOf(el)
    if (!tr) return
    if (tr.dataset.kind === "module") return this.open(tr.dataset.id)
    if (tr.dataset.kind === "function" && !this.diffOpen(tr.dataset.id)) this.toggleDiff(tr.dataset.id)
  },

  // zc
  foldClose(el) {
    const tr = this.rowOf(el)
    if (!tr) return
    if (tr.dataset.kind === "module") return this.collapse(tr.dataset.id)
    if (tr.dataset.kind === "diff") return this.toggleDiff(tr.dataset.parent)
    // a function row: its diff if open, otherwise the module around it
    if (this.diffOpen(tr.dataset.id)) this.toggleDiff(tr.dataset.id)
    else this.collapse(tr.dataset.parent)
  },

  // -- keys ------------------------------------------------------------------------

  handleKey(e) {
    if (e.defaultPrevented) return
    if (e.target.closest("input, textarea, select, [contenteditable]")) return
    if (e.metaKey || e.altKey) return
    if (e.ctrlKey && !["d", "u"].includes(e.key)) return
    // A modifier on its own (Shift before `R`, `G` or `$`) is not a key
    // press; letting it through would clear pending counts and prefixes.
    if (["Shift", "Control", "Alt", "Meta", "CapsLock"].includes(e.key)) return

    const stops = this.stops()
    if (stops.length === 0) return
    const cols = this.columnCount()
    let idx = this.currentIndex(stops)
    const current = stops[idx]
    const clamp = (i) => Math.min(Math.max(i, 0), stops.length - 1)

    // second key of a two-key command
    if (this.pending) {
      const prefix = this.pending
      this.pending = ""
      const hadCount = this.count !== ""
      const n = hadCount ? parseInt(this.count, 10) : 1
      this.count = ""
      let handled = true
      let moved = false

      if (prefix === "g" && e.key === "g") {
        idx = hadCount ? n - 1 : 0
        moved = true
      } else if (prefix === "z") {
        switch (e.key) {
          case "a": this.foldToggle(current); break
          case "o": this.foldOpen(current); break
          case "c": this.foldClose(current); break
          case "R": this.collapsed.clear(); break
          case "M": this.moduleRows().forEach((tr) => this.collapse(tr.dataset.id)); break
          default: handled = false
        }
      } else {
        handled = false
      }

      if (handled) {
        e.preventDefault()
        // motions move by index; fold commands have already placed the cursor
        if (moved) this.stopId = stops[clamp(idx)].dataset.id
        this.render()
      } else {
        this.renderStatus()
      }
      return
    }

    // count prefix: digits accumulate; a leading 0 is the "first column" motion
    if (/^[1-9]$/.test(e.key) || (e.key === "0" && this.count !== "")) {
      this.count += e.key
      e.preventDefault()
      this.renderStatus()
      return
    }

    const n = this.count === "" ? 1 : parseInt(this.count, 10)
    let handled = true

    switch (e.key) {
      case "h": case "ArrowLeft":  this.col -= n; break
      case "l": case "ArrowRight": this.col += n; break
      case "j": case "ArrowDown":  idx += n; break
      case "k": case "ArrowUp":    idx -= n; break
      case "0": case "^": case "Home": this.col = 0; break
      case "$": case "End":        this.col = cols - 1; break
      case "{": idx = this.nthRow(stops, idx, -n); break
      case "}": idx = this.nthRow(stops, idx, n); break
      case "n": idx = this.nthChange(stops, idx, n); break
      case "N": idx = this.nthChange(stops, idx, -n); break
      case "g": case "z":
        this.pending = e.key
        e.preventDefault()
        this.renderStatus()
        return
      case "G": idx = this.count === "" ? stops.length - 1 : n - 1; break
      case "d": if (e.ctrlKey) { idx += this.pageSize() * n } else { handled = false }; break
      case "u": if (e.ctrlKey) { idx -= this.pageSize() * n } else { handled = false }; break
      case "PageDown": idx += this.pageSize() * 2; break
      case "PageUp":   idx -= this.pageSize() * 2; break
      case "Enter": this.activate(current); break
      case "Escape":
        // inside a diff: back out to the row that owns it
        if (this.isLine(current)) idx = this.indexOfId(stops, this.rowOf(current).dataset.parent, idx)
        break
      default: handled = false
    }

    this.count = ""
    if (handled) {
      e.preventDefault()
      // Enter may have moved the cursor itself (closing a diff from inside
      // it); only motions set it from the index.
      if (e.key !== "Enter") this.stopId = stops[clamp(idx)].dataset.id
      this.render()
    } else {
      this.renderStatus()
    }
  },

  activate(stop) {
    if (!stop) return

    if (this.isLine(stop)) {
      const tr = this.rowOf(stop)
      this.pushEvent("activate", {
        id: tr.dataset.parent,
        kind: "line",
        module: tr.dataset.module,
        function: tr.dataset.function || null,
        column: "diff",
        line: {kind: stop.dataset.kind, old: stop.dataset.old || null, new: stop.dataset.new || null},
      })
      return
    }

    const td = stop.children[this.col]
    const column = td ? td.dataset.column : null

    if (stop.dataset.kind === "module" && column === "stats") {
      this.toggleDiff(stop.dataset.id)
      return
    }

    if (stop.dataset.kind === "module") {
      this.toggle(stop.dataset.id)
      return
    }

    if (stop.dataset.kind === "function") {
      this.toggleDiff(stop.dataset.id)
      return
    }

    this.pushEvent("activate", {
      id: stop.dataset.id,
      kind: stop.dataset.kind,
      module: stop.dataset.module,
      function: stop.dataset.function,
      column,
    })
  },

  // -- painting ---------------------------------------------------------------------

  render() {
    // fold state
    for (const tr of this.allRows()) {
      if (tr.dataset.kind === "module") tr.classList.toggle(COLLAPSED, this.collapsed.has(tr.dataset.id))
      else tr.classList.toggle("hidden", this.folded(tr))
    }

    const stops = this.stops()
    if (stops.length === 0) { this.renderStatus(); return }

    const cols = this.columnCount()
    const idx = this.currentIndex(stops)
    this.col = Math.min(Math.max(this.col, 0), Math.max(cols - 1, 0))
    this.stopId = stops[idx].dataset.id
    this.lastIdx = idx

    this.el.querySelectorAll(`.${CURSOR}`).forEach((el) => el.classList.remove(CURSOR))
    this.el.querySelectorAll(`.${CURSOR_ROW}`).forEach((el) => el.classList.remove(CURSOR_ROW))

    const stop = stops[idx]
    stop.classList.add(CURSOR_ROW)
    let target = stop
    if (this.isLine(stop)) {
      // a diff line is one cell wide: the whole line is the cursor
      stop.classList.add(CURSOR)
    } else {
      const td = stop.children[this.col]
      if (td) { td.classList.add(CURSOR); target = td }
    }
    target.scrollIntoView({block: "nearest", inline: "nearest"})

    this.renderStatus()
  },

  renderStatus() {
    const rowEl = this.el.querySelector("[data-vim-row]")
    const totalEl = this.el.querySelector("[data-vim-total]")
    const colLabelEl = this.el.querySelector("[data-vim-col-label]")
    const colEl = this.el.querySelector("[data-vim-col]")
    const pendingEl = this.el.querySelector("[data-vim-pending]")
    const stops = this.stops()
    const idx = this.currentIndex(stops)
    const stop = stops[idx]

    if (rowEl) rowEl.textContent = stops.length ? String(idx + 1) : "–"
    if (totalEl) totalEl.textContent = String(stops.length)
    if (colLabelEl) colLabelEl.textContent = this.isLine(stop) ? "line" : "col"
    if (colEl) colEl.textContent = this.describeColumn(stop)
    if (pendingEl) pendingEl.textContent = (this.count || "") + this.pending
  },

  // `module` for a cell; `+42`, `−17` or `42` for a diff line
  describeColumn(stop) {
    if (!stop) return "–"
    if (this.isLine(stop)) {
      switch (stop.dataset.kind) {
        case "add": return "+" + stop.dataset.new
        case "del": return "−" + stop.dataset.old
        default: return stop.dataset.new || stop.dataset.old || "–"
      }
    }
    const td = stop.children[this.col]
    return td ? (td.dataset.column || String(this.col + 1)) : "–"
  },
}

export default VimGrid
