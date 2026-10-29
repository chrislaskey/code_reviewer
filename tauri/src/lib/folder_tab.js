// The outline of the app's gutter edge with the open tab joined to the page,
// as an SVG path: the page's edge runs top to bottom, and bulges left around
// the open tab, meeting it with concave "feet" like a file folder's tab.

/**
 * @param {object} options
 * @param {number} options.width the drawing's width, the edge plus the overlap under the page
 * @param {number} options.height the drawing's height, the gutter's
 * @param {number} options.edge where the page begins
 * @param {{ left: number, top: number, bottom: number } | null} options.tab the open tab's box, or null for a plain edge
 * @param {number} options.cornerRadius the radius of the tab's left corners
 * @param {number} options.feetRadius the radius of the curves where the tab meets the page; 0 makes them square
 * @returns {string} the path's `d` attribute
 */
export function folderTabPath({ width, height, edge, tab, cornerRadius, feetRadius }) {
  if (tab === null) return `M ${width} 0 L ${width} ${height} L ${edge} ${height} L ${edge} 0 Z`;
  const { left, top, bottom } = tab;
  const r = cornerRadius;
  const f = feetRadius;
  // A tab flush with the top edge keeps a square top corner.
  const rt = top <= 0 ? 0 : r;
  return [
    `M ${width} 0`,
    `L ${width} ${height}`,
    `L ${edge} ${height}`,
    `L ${edge} ${bottom + f}`,
    `A ${f} ${f} 0 0 0 ${edge - f} ${bottom}`,
    `L ${left + r} ${bottom}`,
    `A ${r} ${r} 0 0 1 ${left} ${bottom - r}`,
    `L ${left} ${top + rt}`,
    `A ${rt} ${rt} 0 0 1 ${left + rt} ${top}`,
    `L ${edge - f} ${top}`,
    `A ${f} ${f} 0 0 0 ${edge} ${top - f}`,
    `L ${edge} 0`,
    "Z",
  ].join(" ");
}
