// VimFiles: keyboard cursor over the files view, which has two panes: the
// file tree on the left and the diff cards on the right.
//
// The cursor lives in one pane at a time. In the tree it sits on a file
// (a[data-file]); in the diff it sits on a line ([data-line] inside an
// article). The file is remembered across panes: `l` steps from a file into
// its diff, `h` steps back out onto the same file, and moving through lines
// on the right keeps the tree highlight on whichever file the line belongs
// to. Cards fold on the server; `updated()` re-applies the cursor after
// every re-render.
//
//   j / k, arrows     tree: previous / next file
//                     diff: previous / next line, flowing across files
//   Enter             tree: scroll the diff to the highlighted file
//                     diff: "activate" the line on the server
//   l / →             into the diff of the highlighted file (unfolding it)
//   h / ← / Escape    back to the tree
//   n / N             next / previous change (a run of +/− lines)
//   { / }             first line of the previous / next file
//   gg / G            first / last (3G = third), counts as a prefix: 5j
//   Ctrl-d / Ctrl-u   half page down / up
//   za / zo / zc      toggle / open / close the highlighted file's diff
//   zR / zM           expand / collapse every file

const CURSOR = "is-cursor"
const ACTIVE = "is-active-file"

const VimFiles = {
  mounted() {
    this.pane = "tree" // "tree" | "diff"
    this.fileId = null // the highlighted file: an <article> id and a tree link's data-file
    this.lineId = null // the highlighted diff line, when in the diff pane
    this.count = ""
    this.pending = "" // "g" or "z" awaiting a second key

    this.onKeydown = (e) => this.handleKey(e)
    window.addEventListener("keydown", this.onKeydown)

    // a click is the cursor landing there
    this.el.addEventListener("click", (e) => {
      const link = e.target.closest("#file-tree a[data-file]")
      if (link) {
        this.pane = "tree"
        this.fileId = link.dataset.file
        this.render()
        return
      }

      const line = e.target.closest("#file-cards [data-line]")
      if (line) {
        this.pane = "diff"
        this.lineId = line.dataset.id
        this.fileId = this.cardOf(line).id
        this.render()
      }
    })

    this.render()
  },

  updated() {
    this.render()
  },

  destroyed() {
    window.removeEventListener("keydown", this.onKeydown)
  },

  // -- stops -------------------------------------------------------------------

  // Files in the tree, skipping those inside a closed directory.
  treeStops() {
    return Array.from(this.el.querySelectorAll("#file-tree a[data-file]")).filter(
      (a) => !a.closest("details:not([open])"),
    )
  },

  // Every diff line on the right, in display order. Folded cards have none.
  lineStops() {
    return Array.from(this.el.querySelectorAll("#file-cards article [data-line]"))
  },

  card(fileId) {
    return fileId ? this.el.querySelector(`#file-cards article[id="${CSS.escape(fileId)}"]`) : null
  },

  cardOf(line) {
    return line.closest("article")
  },

  linesIn(card) {
    return card ? Array.from(card.querySelectorAll("[data-line]")) : []
  },

  folded(card) {
    return !!card && card.hasAttribute("data-collapsed")
  },

  indexOf(stops, key, id, fallback = 0) {
    const idx = stops.findIndex((el) => el.dataset[key] === id)
    return idx === -1 ? Math.min(fallback, Math.max(stops.length - 1, 0)) : idx
  },

  pageSize() {
    const line = this.el.querySelector("#file-cards [data-line]")
    const height = line ? line.getBoundingClientRect().height || 20 : 28
    return Math.max(1, Math.floor(window.innerHeight / height / 2))
  },

  // -- motions -----------------------------------------------------------------

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

  isChange(el) {
    return !!el && (el.dataset.kind === "add" || el.dataset.kind === "del")
  },

  // n and N: the first line of each run of +/− lines
  nthChange(stops, idx, n) {
    return this.nth(stops, idx, n, (el, i) => this.isChange(el) && !this.isChange(stops[i - 1]))
  },

  // { and }: the first line of each file
  nthFile(stops, idx, n) {
    return this.nth(stops, idx, n, (el, i) => i === 0 || this.cardOf(el) !== this.cardOf(stops[i - 1]))
  },

  // -- panes -------------------------------------------------------------------

  // `l`: into the highlighted file's diff. A folded card is asked to open;
  // its lines arrive with the next update and render() lands on the first.
  enterDiff() {
    const card = this.card(this.fileId)
    if (!card) return
    if (this.folded(card)) this.pushEvent("toggle_collapsed", {id: card.id})
    this.pane = "diff"
    const lines = this.linesIn(card)
    // resume where the cursor was in this file, else start at the top
    if (!lines.some((l) => l.dataset.id === this.lineId)) this.lineId = lines[0] ? lines[0].dataset.id : null
    if (lines.length === 0) this.revealCard(card)
  },

  // `h`: back onto the file in the tree.
  leaveDiff() {
    this.pane = "tree"
  },

  // -- folds -------------------------------------------------------------------

  foldToggle() {
    const card = this.card(this.fileId)
    if (!card) return
    if (this.folded(card)) this.foldOpen()
    else this.foldClose()
  },

  foldOpen() {
    const card = this.card(this.fileId)
    if (card && this.folded(card)) this.pushEvent("toggle_collapsed", {id: card.id})
  },

  foldClose() {
    const card = this.card(this.fileId)
    if (card && !this.folded(card)) this.pushEvent("toggle_collapsed", {id: card.id})
    // the lines under the cursor are going away
    this.leaveDiff()
  },

  // -- keys --------------------------------------------------------------------

  handleKey(e) {
    if (e.defaultPrevented) return
    if (e.target.closest("input, textarea, select, [contenteditable]")) return
    if (e.metaKey || e.altKey) return
    if (e.ctrlKey && !["d", "u"].includes(e.key)) return
    if (["Shift", "Control", "Alt", "Meta", "CapsLock"].includes(e.key)) return

    // second key of a two-key command
    if (this.pending) {
      const prefix = this.pending
      this.pending = ""
      const hadCount = this.count !== ""
      const n = hadCount ? parseInt(this.count, 10) : 1
      this.count = ""
      let handled = true

      if (prefix === "g" && e.key === "g") {
        this.moveTo(hadCount ? n - 1 : 0)
      } else if (prefix === "z") {
        switch (e.key) {
          case "a": this.foldToggle(); break
          case "o": this.foldOpen(); break
          case "c": this.foldClose(); break
          case "R": this.pushEvent("expand_all", {}); break
          case "M": this.pushEvent("collapse_all", {}); this.leaveDiff(); break
          default: handled = false
        }
      } else {
        handled = false
      }

      if (handled) e.preventDefault()
      this.render()
      return
    }

    // count prefix
    if (/^[1-9]$/.test(e.key) || (e.key === "0" && this.count !== "")) {
      this.count += e.key
      e.preventDefault()
      this.renderStatus()
      return
    }

    if (e.key === "g" || e.key === "z") {
      this.pending = e.key
      e.preventDefault()
      this.renderStatus()
      return
    }

    const hadCount = this.count !== ""
    const n = hadCount ? parseInt(this.count, 10) : 1
    this.count = ""
    const handled = this.pane === "tree" ? this.treeKey(e, n, hadCount) : this.diffKey(e, n, hadCount)

    if (handled) {
      e.preventDefault()
      this.render()
    } else {
      this.renderStatus()
    }
  },

  treeKey(e, n, hadCount) {
    const stops = this.treeStops()
    if (stops.length === 0) return false
    let idx = this.indexOf(stops, "file", this.fileId)
    const clamp = (i) => Math.min(Math.max(i, 0), stops.length - 1)

    switch (e.key) {
      case "j": case "ArrowDown": idx += n; break
      case "k": case "ArrowUp":   idx -= n; break
      case "G": idx = hadCount ? n - 1 : stops.length - 1; break
      case "d": idx += this.pageSize() * n; break
      case "u": idx -= this.pageSize() * n; break
      case "PageDown": idx += this.pageSize() * 2; break
      case "PageUp":   idx -= this.pageSize() * 2; break
      case "Enter":
        this.revealCard(this.card(stops[idx].dataset.file))
        return true
      case "l": case "ArrowRight":
        this.enterDiff()
        return true
      case "Escape":
        return true
      default:
        return false
    }

    this.fileId = stops[clamp(idx)].dataset.file
    return true
  },

  diffKey(e, n, hadCount) {
    if (["h", "ArrowLeft", "Escape"].includes(e.key)) {
      this.leaveDiff()
      return true
    }

    const stops = this.lineStops()
    if (stops.length === 0) return false
    let idx = this.indexOf(stops, "id", this.lineId)
    const clamp = (i) => Math.min(Math.max(i, 0), stops.length - 1)

    switch (e.key) {
      case "j": case "ArrowDown": idx += n; break
      case "k": case "ArrowUp":   idx -= n; break
      case "G": idx = hadCount ? n - 1 : stops.length - 1; break
      case "d": idx += this.pageSize() * n; break
      case "u": idx -= this.pageSize() * n; break
      case "PageDown": idx += this.pageSize() * 2; break
      case "PageUp":   idx -= this.pageSize() * 2; break
      case "n": idx = this.nthChange(stops, idx, n); break
      case "N": idx = this.nthChange(stops, idx, -n); break
      case "}": idx = this.nthFile(stops, idx, n); break
      case "{": idx = this.nthFile(stops, idx, -n); break
      case "Enter":
        this.activate(stops[idx])
        return true
      case "l": case "ArrowRight":
        return true
      default:
        return false
    }

    this.lineId = stops[clamp(idx)].dataset.id
    return true
  },

  // gg and G land on an absolute index in the current pane.
  moveTo(idx) {
    if (this.pane === "tree") {
      const stops = this.treeStops()
      if (stops.length) this.fileId = stops[Math.min(Math.max(idx, 0), stops.length - 1)].dataset.file
    } else {
      const stops = this.lineStops()
      if (stops.length) this.lineId = stops[Math.min(Math.max(idx, 0), stops.length - 1)].dataset.id
    }
  },

  activate(line) {
    if (!line) return
    const card = this.cardOf(line)
    this.pushEvent("activate", {
      id: card.id,
      path: card.dataset.path,
      line: {kind: line.dataset.kind, old: line.dataset.old || null, new: line.dataset.new || null},
    })
  },

  // -- painting ----------------------------------------------------------------

  render() {
    this.el.classList.toggle("pane-tree", this.pane === "tree")
    this.el.classList.toggle("pane-diff", this.pane === "diff")
    this.el.querySelectorAll(`.${CURSOR}`).forEach((el) => el.classList.remove(CURSOR))
    this.el.querySelectorAll(`.${ACTIVE}`).forEach((el) => el.classList.remove(ACTIVE))

    const tree = this.treeStops()
    if (!this.fileId && tree[0]) this.fileId = tree[0].dataset.file

    if (this.pane === "diff") {
      // the line under the cursor, or the top of the file when it is gone
      // (a card that just unfolded or was re-rendered)
      let line = this.lineStops().find((l) => l.dataset.id === this.lineId)
      if (!line) line = this.linesIn(this.card(this.fileId))[0] || null
      if (line) {
        this.lineId = line.dataset.id
        this.fileId = this.cardOf(line).id
        line.classList.add(CURSOR)
        this.revealLine(line)
      }
    }

    const link = tree.find((a) => a.dataset.file === this.fileId)
    if (link) {
      link.classList.add(CURSOR)
      if (this.pane === "tree") link.scrollIntoView({block: "nearest"})
    }

    const card = this.card(this.fileId)
    if (card) card.classList.add(ACTIVE)

    this.renderStatus()
  },

  revealCard(card) {
    if (card) card.scrollIntoView({block: "start"})
  },

  // Bring a line into view, then nudge it out from under its card's sticky
  // header if that is where it landed.
  revealLine(line) {
    line.scrollIntoView({block: "nearest"})
    const header = this.cardOf(line).querySelector(":scope > header")
    if (!header) return
    const gap = header.getBoundingClientRect().bottom - line.getBoundingClientRect().top
    if (gap > 0) window.scrollBy(0, -gap)
  },

  renderStatus() {
    const set = (sel, text) => {
      const el = this.el.querySelector(sel)
      if (el) el.textContent = text
    }

    if (this.pane === "tree") {
      const stops = this.treeStops()
      const idx = this.indexOf(stops, "file", this.fileId)
      set("[data-vim-pane]", "files")
      set("[data-vim-pos-label]", "file")
      set("[data-vim-pos]", stops.length ? String(idx + 1) : "–")
      set("[data-vim-total]", String(stops.length))
      set("[data-vim-detail]", "")
    } else {
      const stops = this.lineStops()
      const idx = stops.findIndex((l) => l.dataset.id === this.lineId)
      const line = stops[idx]
      set("[data-vim-pane]", "diff")
      set("[data-vim-pos-label]", "line")
      set("[data-vim-pos]", line ? String(idx + 1) : "–")
      set("[data-vim-total]", String(stops.length))
      set("[data-vim-detail]", this.describeLine(line))
    }

    set("[data-vim-pending]", (this.count || "") + this.pending)
  },

  // `+42` for an added line, `−17` for a removed one, `42` for context
  describeLine(line) {
    if (!line) return ""
    switch (line.dataset.kind) {
      case "add": return "+" + line.dataset.new
      case "del": return "−" + line.dataset.old
      default: return line.dataset.new || line.dataset.old || ""
    }
  },
}

export default VimFiles
