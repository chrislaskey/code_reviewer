// VimGrid: keyboard cursor over a <table> inside the hook element, with
// fold-style expand/collapse of module rows.
//
// Rows are either module rows (data-kind="module") or function rows that
// point at their module (data-parent). Collapsed modules hide their function
// rows; the cursor only ever visits visible rows. All of this is client
// side so it stays instant; only Enter on a non-module cell talks to the
// server ("activate"). The server re-renders freely: `updated()` re-applies
// cursor and fold state.
//
//   h j k l / arrows   move by one (prefix a count: 5j)
//   gg / G             first / last row (3G = row 3)
//   0 ^ / $            first / last column
//   Ctrl-d / Ctrl-u    half page down / up
//   Enter              on a module row: toggle its functions
//                      on a module row's +/- cell: toggle the whole-module diff
//                      on a function row: toggle its diff row
//   za / zo / zc       toggle / open / close the module under the cursor
//   zR / zM            open / close every module
//   Escape             clear pending keys

const CURSOR = "is-cursor"
const CURSOR_ROW = "is-cursor-row"
const COLLAPSED = "is-collapsed"

const VimGrid = {
  mounted() {
    this.rowId = null
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
      const td = e.target.closest("tbody td")
      if (!td || !this.el.contains(td)) return
      const tr = td.parentElement
      if (!tr.dataset.id || tr.dataset.kind === "diff") return
      this.rowId = tr.dataset.id
      this.col = td.cellIndex
      // a click is Enter on that cell
      this.activate(tr)
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

  allRows() {
    return Array.from(this.el.querySelectorAll("tbody tr[data-id]"))
  },

  moduleRows() {
    return this.allRows().filter((tr) => tr.dataset.kind === "module")
  },

  // Navigable rows: not diffs, and not functions of a collapsed module.
  // Diff rows are scenery: hidden with their module's fold, never a cursor stop.
  rows() {
    return this.allRows().filter(
      (tr) => tr.dataset.kind !== "diff" && !(tr.dataset.parent && this.collapsed.has(tr.dataset.parent))
    )
  },

  columnCount() {
    const first = this.rows()[0]
    return first ? first.children.length : 0
  },

  currentIndex(rows) {
    const idx = rows.findIndex((tr) => tr.dataset.id === this.rowId)
    return idx === -1 ? 0 : idx
  },

  pageSize() {
    const tr = this.rows()[0]
    const rowHeight = tr ? tr.getBoundingClientRect().height || 28 : 28
    return Math.max(1, Math.floor(window.innerHeight / rowHeight / 2))
  },

  // -- folds -------------------------------------------------------------------

  moduleIdFor(tr) {
    if (!tr) return null
    return tr.dataset.kind === "module" ? tr.dataset.id : tr.dataset.parent
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
    const current = this.allRows().find((tr) => tr.dataset.id === this.rowId)
    if (current && current.dataset.parent === moduleId) this.rowId = moduleId
  },

  open(moduleId) {
    if (moduleId) this.collapsed.delete(moduleId)
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

    const rows = this.rows()
    if (rows.length === 0) return
    const cols = this.columnCount()
    let idx = this.currentIndex(rows)
    const current = rows[idx]

    // second key of a two-key command
    if (this.pending) {
      const prefix = this.pending
      this.pending = ""
      const n = this.count === "" ? 1 : parseInt(this.count, 10)
      this.count = ""
      let handled = true

      if (prefix === "g" && e.key === "g") {
        idx = this.count === "" ? 0 : n - 1
      } else if (prefix === "z") {
        const id = this.moduleIdFor(current)
        switch (e.key) {
          case "a": this.toggle(id); break
          case "o": this.open(id); break
          case "c": this.collapse(id); break
          case "R": this.collapsed.clear(); break
          case "M": this.moduleRows().forEach((tr) => this.collapse(tr.dataset.id)); break
          default: handled = false
        }
      } else {
        handled = false
      }

      if (handled) {
        e.preventDefault()
        // `gg` moves by index; fold commands keep the cursor on its row
        // (collapse() has already moved it to the module row if needed)
        if (prefix === "g") this.rowId = rows[Math.min(Math.max(idx, 0), rows.length - 1)].dataset.id
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
      case "g": case "z":
        this.pending = e.key
        e.preventDefault()
        this.renderStatus()
        return
      case "G": idx = this.count === "" ? rows.length - 1 : n - 1; break
      case "d": if (e.ctrlKey) { idx += this.pageSize() * n } else { handled = false }; break
      case "u": if (e.ctrlKey) { idx -= this.pageSize() * n } else { handled = false }; break
      case "PageDown": idx += this.pageSize() * 2; break
      case "PageUp":   idx -= this.pageSize() * 2; break
      case "Enter": this.activate(current); break
      case "Escape": break
      default: handled = false
    }

    this.count = ""
    if (handled) {
      e.preventDefault()
      idx = Math.min(Math.max(idx, 0), rows.length - 1)
      this.rowId = rows[idx].dataset.id
      this.render()
    } else {
      this.renderStatus()
    }
  },

  activate(tr) {
    if (!tr) return
    const td = tr.children[this.col]
    const column = td ? td.dataset.column : null

    if (tr.dataset.kind === "module" && column === "stats") {
      this.pushEvent("toggle_diff", {id: tr.dataset.id})
      return
    }

    if (tr.dataset.kind === "module") {
      this.toggle(tr.dataset.id)
      return
    }

    if (tr.dataset.kind === "function") {
      this.pushEvent("toggle_diff", {id: tr.dataset.id})
      return
    }

    this.pushEvent("activate", {
      id: tr.dataset.id,
      kind: tr.dataset.kind,
      module: tr.dataset.module,
      function: tr.dataset.function,
      column,
    })
  },

  // -- painting ---------------------------------------------------------------------

  render() {
    // fold state
    for (const tr of this.allRows()) {
      if (tr.dataset.kind === "module") {
        tr.classList.toggle(COLLAPSED, this.collapsed.has(tr.dataset.id))
      } else if (tr.dataset.kind === "diff") {
        tr.classList.toggle("hidden", this.collapsed.has(tr.dataset.fold))
      } else if (tr.dataset.parent) {
        tr.classList.toggle("hidden", this.collapsed.has(tr.dataset.parent))
      }
    }

    const rows = this.rows()
    if (rows.length === 0) { this.renderStatus(); return }

    const cols = this.columnCount()
    const idx = this.currentIndex(rows)
    this.col = Math.min(Math.max(this.col, 0), cols - 1)
    this.rowId = rows[idx].dataset.id

    this.el.querySelectorAll(`.${CURSOR}`).forEach((el) => el.classList.remove(CURSOR))
    this.el.querySelectorAll(`.${CURSOR_ROW}`).forEach((el) => el.classList.remove(CURSOR_ROW))

    const tr = rows[idx]
    const td = tr.children[this.col]
    tr.classList.add(CURSOR_ROW)
    if (td) {
      td.classList.add(CURSOR)
      td.scrollIntoView({block: "nearest", inline: "nearest"})
    }

    this.renderStatus()
  },

  renderStatus() {
    const rowEl = this.el.querySelector("[data-vim-row]")
    const totalEl = this.el.querySelector("[data-vim-total]")
    const colEl = this.el.querySelector("[data-vim-col]")
    const pendingEl = this.el.querySelector("[data-vim-pending]")
    const rows = this.rows()
    const tr = rows[this.currentIndex(rows)]
    const td = tr ? tr.children[this.col] : null

    if (rowEl) rowEl.textContent = rows.length ? String(this.currentIndex(rows) + 1) : "–"
    if (totalEl) totalEl.textContent = String(rows.length)
    if (colEl) colEl.textContent = td ? (td.dataset.column || String(this.col + 1)) : "–"
    if (pendingEl) pendingEl.textContent = (this.count || "") + this.pending
  },
}

export default VimGrid
