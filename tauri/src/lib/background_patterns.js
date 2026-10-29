// Background patterns, used at the top of pages and tried out on the
// experiments page. Every pattern is a list of mask layers: the layer is
// filled with a colour and the mask cuts the shapes, so any theme colour
// shows through. A tiled pattern returns CSS mask images to repeat; a
// generated one draws one SVG for the whole area.

import alea from "alea";
import { createNoise2D } from "simplex-noise";

/** A data URL of an SVG, for a mask image. */
function svgUrl(width, height, body) {
  const svg = `<svg xmlns='http://www.w3.org/2000/svg' width='${width}' height='${height}' viewBox='0 0 ${width} ${height}'>${body}</svg>`;
  return `url("data:image/svg+xml,${encodeURIComponent(svg)}")`;
}

function stroke(width = 1.2) {
  return `fill='none' stroke='#000' stroke-width='${width}' stroke-linecap='round' stroke-linejoin='round'`;
}

function tile(width, height, body) {
  return { image: svgUrl(width, height, body), size: `${width}px ${height}px` };
}

function dot(radius) {
  return `radial-gradient(circle, #000 ${radius}px, transparent ${radius + 0.6}px)`;
}

function plus(x, y, arm) {
  return `M${x - arm} ${y}H${x + arm}M${x} ${y - arm}V${y + arm}`;
}

function minus(x, y, arm) {
  return `M${x - arm} ${y}H${x + arm}`;
}

/** The six edges of a pointy-top hexagon of the given radius. */
function hexagon(cx, cy, r) {
  const points = [30, 90, 150, 210, 270, 330].map((degrees) => {
    const radians = (degrees * Math.PI) / 180;
    return `${(cx + r * Math.cos(radians)).toFixed(2)} ${(cy + r * Math.sin(radians)).toFixed(2)}`;
  });
  return `M${points.join("L")}Z`;
}

/** The points of a square lattice turned by `angle`, covering the area with a margin. */
function rotatedLattice(width, height, spacing, angle) {
  const radians = (angle * Math.PI) / 180;
  const cos = Math.cos(radians);
  const sin = Math.sin(radians);
  const reach = Math.ceil(Math.hypot(width, height) / spacing / 2) + 1;
  const points = [];
  for (let i = -reach; i <= reach; i++) {
    for (let j = -reach; j <= reach; j++) {
      const x = width / 2 + i * spacing * cos - j * spacing * sin;
      const y = height / 2 + i * spacing * sin + j * spacing * cos;
      if (x > -spacing && x < width + spacing && y > -spacing && y < height + spacing) points.push({ i, j, x, y });
    }
  }
  return points;
}

/**
 * @typedef {{ tone?: "ink" | "success" | "error" | "warning", image: string, size?: string, position?: string }} MaskLayer
 * @typedef {{ size: number, width: number, height: number, angle: number }} PatternSettings
 * @typedef {{ id: string, name: string, idea: string, generated?: boolean, animated?: boolean, color?: string,
 *   layers: (settings: PatternSettings) => MaskLayer[] }} Pattern
 */

/** @type {Pattern[]} */
export const PATTERNS = [
  {
    id: "dots",
    name: "Dot grid",
    idea: "The classic: small dots on a square grid, like the code canvas.",
    layers: ({ size }) => [{ image: dot(Math.max(1, size / 18)), size: `${size}px ${size}px` }],
  },
  {
    id: "fine-dots",
    name: "Fine dots",
    idea: "Half the spacing and smaller dots, so it reads as a texture.",
    layers: ({ size }) => [{ image: dot(0.8), size: `${size / 2}px ${size / 2}px` }],
  },
  {
    id: "staggered-dots",
    name: "Staggered dots",
    idea: "Every other row shifted by half, a diagonal lattice.",
    layers: ({ size }) => [
      {
        image: `${dot(Math.max(1, size / 18))}, ${dot(Math.max(1, size / 18))}`,
        size: `${size}px ${size}px`,
        position: `0 0, ${size / 2}px ${size / 2}px`,
      },
    ],
  },
  {
    id: "polka",
    name: "Polka dots",
    idea: "Big soft dots. Bolder, works best faint.",
    layers: ({ size }) => [{ image: dot(size * 0.16), size: `${size}px ${size}px` }],
  },
  {
    id: "halftone",
    name: "Halftone",
    idea: "Dots shrink as they go down, so the pattern fades by itself like print halftone.",
    generated: true,
    layers: ({ size, width, height, angle }) => {
      const circles = rotatedLattice(width, height, size, angle)
        .map(({ x, y }) => ({ x, y, r: size * 0.3 * Math.max(0, 1 - y / height) ** 1.3 }))
        .filter(({ r }) => r > 0.35)
        .map(({ x, y, r }) => `<circle cx='${x.toFixed(1)}' cy='${y.toFixed(1)}' r='${r.toFixed(2)}'/>`);
      return [{ image: svgUrl(width, height, circles.join("")) }];
    },
  },
  {
    id: "gradient-dots",
    name: "Gradient dots",
    idea: "The dot grid filled with the theme's primary, secondary and accent colours.",
    color: "gradient",
    layers: ({ size }) => [{ image: dot(Math.max(1.2, size / 14)), size: `${size}px ${size}px` }],
  },
  {
    id: "drifting-dots",
    name: "Drifting dots",
    idea: "The dot grid sliding very slowly along its angle. Still when the system asks for less motion.",
    animated: true,
    layers: ({ size }) => [{ image: dot(Math.max(1, size / 18)), size: `${size}px ${size}px` }],
  },
  {
    id: "scattered",
    name: "Scattered dots",
    idea: "A grid with each dot nudged and sized at random: less rigid, like stars.",
    generated: true,
    layers: ({ size, width, height, angle }) => {
      const random = alea("scattered");
      const circles = rotatedLattice(width, height, size, angle).map(({ x, y }) => {
        const jx = x + (random() - 0.5) * size * 0.7;
        const jy = y + (random() - 0.5) * size * 0.7;
        const r = 0.5 + random() ** 2 * 2;
        return `<circle cx='${jx.toFixed(1)}' cy='${jy.toFixed(1)}' r='${r.toFixed(2)}'/>`;
      });
      return [{ image: svgUrl(width, height, circles.join("")) }];
    },
  },
  {
    id: "constellation",
    name: "Constellation",
    idea: "Nudged dots, some joined to a neighbour: a network, or a dependency graph.",
    generated: true,
    layers: ({ size, width, height, angle }) => {
      const random = alea("constellation");
      const spacing = size * 1.6;
      const points = rotatedLattice(width, height, spacing, angle).map((point) => ({
        ...point,
        x: point.x + (random() - 0.5) * spacing * 0.6,
        y: point.y + (random() - 0.5) * spacing * 0.6,
      }));
      const byLattice = new Map(points.map((point) => [`${point.i},${point.j}`, point]));
      const lines = [];
      for (const point of points) {
        for (const [di, dj] of [[1, 0], [0, 1], [1, 1]]) {
          const neighbour = byLattice.get(`${point.i + di},${point.j + dj}`);
          if (neighbour && random() < 0.28) {
            lines.push(`M${point.x.toFixed(1)} ${point.y.toFixed(1)}L${neighbour.x.toFixed(1)} ${neighbour.y.toFixed(1)}`);
          }
        }
      }
      const circles = points.map(({ x, y }) => `<circle cx='${x.toFixed(1)}' cy='${y.toFixed(1)}' r='1.8'/>`);
      const body = `<path d='${lines.join("")}' stroke='#000' stroke-opacity='0.55' stroke-width='0.8'/>${circles.join("")}`;
      return [{ image: svgUrl(width, height, body) }];
    },
  },
  {
    id: "plus",
    name: "Plus signs",
    idea: "Small + marks on a grid, like registration marks on a print sheet.",
    layers: ({ size }) => [tile(size, size, `<path d='${plus(size / 2, size / 2, size * 0.14)}' ${stroke()}/>`)],
  },
  {
    id: "crosses",
    name: "Crosses",
    idea: "× marks, the plus signs turned 45°.",
    layers: ({ size }) => {
      const c = size / 2;
      const a = size * 0.1;
      return [tile(size, size, `<path d='M${c - a} ${c - a}L${c + a} ${c + a}M${c + a} ${c - a}L${c - a} ${c + a}' ${stroke()}/>`)];
    },
  },
  {
    id: "diff-glyphs",
    name: "Diff marks",
    idea: "Alternating + and − in the diff colours. On theme for a code reviewer.",
    layers: ({ size }) => {
      const s = size;
      const arm = s * 0.14;
      return [
        { tone: "success", ...tile(2 * s, 2 * s, `<path d='${plus(s / 2, s / 2, arm)}${plus(1.5 * s, 1.5 * s, arm)}' ${stroke()}/>`) },
        { tone: "error", ...tile(2 * s, 2 * s, `<path d='${minus(1.5 * s, s / 2, arm)}${minus(s / 2, 1.5 * s, arm)}' ${stroke()}/>`) },
      ];
    },
  },
  {
    id: "code-lines",
    name: "Code silhouette",
    idea: "Rounded bars indented like lines of code, with no text. Reads as code from afar.",
    layers: ({ size }) => {
      const random = alea("code-lines");
      const rowHeight = size * 0.55;
      const width = size * 14;
      const rows = 16;
      const bars = [];
      let indent = 0;
      for (let row = 0; row < rows; row++) {
        indent = Math.max(0, Math.min(3, indent + Math.floor(random() * 3) - 1));
        if (random() < 0.12) continue;
        let x = size * 0.5 + indent * size * 0.9;
        const y = row * rowHeight + rowHeight / 2;
        for (let word = 0; word < 1 + Math.floor(random() * 3); word++) {
          const length = size * (0.6 + random() * 2.2);
          if (x + length > width - size * 0.5) break;
          bars.push(`M${x.toFixed(1)} ${y.toFixed(1)}h${length.toFixed(1)}`);
          x += length + size * 0.35;
        }
      }
      const thickness = Math.max(2, size * 0.13);
      return [tile(width, rows * rowHeight, `<path d='${bars.join("")}' stroke='#000' stroke-width='${thickness}' stroke-linecap='round'/>`)];
    },
  },
  {
    id: "commit-graph",
    name: "Commit graph",
    idea: "A branch leaving a line and merging back, tiled, like `git log --graph`.",
    layers: ({ size }) => {
      const s = size;
      const path =
        `M${s / 2} 0V${3 * s}` +
        `M${s / 2} ${s * 0.5}C${s / 2} ${s * 0.95} ${1.5 * s} ${s * 0.75} ${1.5 * s} ${s * 1.2}` +
        `V${s * 1.8}C${1.5 * s} ${s * 2.25} ${s / 2} ${s * 2.05} ${s / 2} ${s * 2.5}`;
      const dots = [[0.5, 0.5], [0.5, 1.5], [1.5, 1.5], [0.5, 2.5]]
        .map(([x, y]) => `<circle cx='${x * s}' cy='${y * s}' r='${Math.max(2, s * 0.1)}'/>`)
        .join("");
      return [tile(2 * s, 3 * s, `<path d='${path}' ${stroke()}/>${dots}`)];
    },
  },
  {
    id: "graph-paper",
    name: "Graph paper",
    idea: "Thin lines on a square grid.",
    layers: ({ size }) => [
      {
        image: "linear-gradient(#000 1px, transparent 1px), linear-gradient(90deg, #000 1px, transparent 1px)",
        size: `${size}px ${size}px`,
      },
    ],
  },
  {
    id: "blueprint",
    name: "Blueprint",
    idea: "A faint minor grid with a stronger line every fifth, like engineering paper.",
    layers: ({ size }) => [
      {
        image:
          "linear-gradient(#000 1px, transparent 1px), linear-gradient(90deg, #000 1px, transparent 1px), " +
          "linear-gradient(rgb(0 0 0 / 0.35) 1px, transparent 1px), linear-gradient(90deg, rgb(0 0 0 / 0.35) 1px, transparent 1px)",
        size: `${size * 5}px ${size * 5}px, ${size * 5}px ${size * 5}px, ${size}px ${size}px, ${size}px ${size}px`,
      },
    ],
  },
  {
    id: "pinstripes",
    name: "Pinstripes",
    idea: "Thin parallel lines; the angle setting does all the work.",
    layers: ({ size }) => [{ image: `repeating-linear-gradient(90deg, #000 0 1px, transparent 1px ${size / 2}px)`, size: "auto" }],
  },
  {
    id: "dashes",
    name: "Dashes",
    idea: "Short dashes in staggered rows, like stitching.",
    layers: ({ size }) => [
      tile(size, size, `<path d='M${size * 0.1} ${size / 4}h${size * 0.3}M${size * 0.6} ${(3 * size) / 4}h${size * 0.3}' ${stroke()}/>`),
    ],
  },
  {
    id: "triangles",
    name: "Triangles",
    idea: "An equilateral triangle grid.",
    layers: ({ size }) => {
      const s = size * 1.2;
      const h = s * Math.sqrt(3);
      return [tile(s, h, `<path d='M0 0H${s}M0 ${h / 2}H${s}M0 0L${s} ${h}M${s} 0L0 ${h}' ${stroke(0.9)}/>`)];
    },
  },
  {
    id: "honeycomb",
    name: "Honeycomb",
    idea: "Hexagon outlines.",
    layers: ({ size }) => {
      const r = size * 0.6;
      const w = Math.sqrt(3) * r;
      const h = 3 * r;
      const centres = [[0, 0], [w, 0], [w / 2, 1.5 * r], [0, h], [w, h]];
      const path = centres.map(([x, y]) => hexagon(x, y, r)).join("");
      return [tile(Number(w.toFixed(2)), Number(h.toFixed(2)), `<path d='${path}' ${stroke(0.9)}/>`)];
    },
  },
  {
    id: "diamonds",
    name: "Diamonds",
    idea: "Diamond outlines, set apart from each other.",
    layers: ({ size }) => {
      const c = size / 2;
      const a = size * 0.28;
      return [tile(size, size, `<path d='M${c} ${c - a}L${c + a} ${c}L${c} ${c + a}L${c - a} ${c}Z' ${stroke()}/>`)];
    },
  },
  {
    id: "rings",
    name: "Rings",
    idea: "Small circle outlines.",
    layers: ({ size }) => {
      const r = size * 0.22;
      return [{ image: `radial-gradient(circle, transparent ${r - 1}px, #000 ${r - 1}px, #000 ${r}px, transparent ${r + 0.6}px)`, size: `${size}px ${size}px` }];
    },
  },
  {
    id: "checkerboard",
    name: "Checkerboard",
    idea: "Filled squares. Only works very faint.",
    layers: ({ size }) => [
      { image: "conic-gradient(#000 25%, transparent 0 50%, #000 0 75%, transparent 0)", size: `${size}px ${size}px` },
    ],
  },
  {
    id: "waves",
    name: "Wavy lines",
    idea: "Rows of gentle waves, a calmer echo of the waves at the bottom.",
    layers: ({ size }) => {
      const w = size * 2;
      const h = size * 0.75;
      const a = size * 0.18;
      return [tile(w, h, `<path d='M0 ${h / 2}Q${w / 4} ${h / 2 - a} ${w / 2} ${h / 2}T${w} ${h / 2}' ${stroke()}/>`)];
    },
  },
  {
    id: "zigzag",
    name: "Zigzag",
    idea: "Rows of chevrons.",
    layers: ({ size }) => {
      const h = size / 2;
      return [tile(size, h, `<path d='M0 ${h * 0.75}L${size / 2} ${h * 0.25}L${size} ${h * 0.75}' ${stroke()}/>`)];
    },
  },
  {
    id: "flow-lines",
    name: "Flow lines",
    idea: "Lines bent by noise, like a topographic map. Ignores the angle.",
    generated: true,
    layers: ({ size, width, height }) => {
      const noise = createNoise2D(alea("flow-lines"));
      const gap = size * 0.6;
      const paths = [];
      for (let row = 0, y = gap / 2; y < height + gap; row++, y += gap) {
        const points = [];
        for (let x = 0; x <= width + 8; x += 8) {
          points.push(`${x} ${(y + noise(x * 0.003, row * 0.12) * size * 1.4).toFixed(1)}`);
        }
        paths.push(`M${points.join("L")}`);
      }
      return [{ image: svgUrl(width, height, `<path d='${paths.join("")}' fill='none' stroke='#000' stroke-width='1'/>`) }];
    },
  },
];

/** How the pattern fades out, as a CSS mask on the area it covers. */
export const FADES = [
  { value: "top", label: "Top", mask: "linear-gradient(to bottom, #000 0%, rgb(0 0 0 / 0.6) 35%, transparent 100%)" },
  { value: "spotlight", label: "Spotlight", mask: "radial-gradient(ellipse 60% 100% at 50% 0%, #000 0%, transparent 100%)" },
  { value: "corner", label: "Corner", mask: "radial-gradient(circle at 100% 0%, #000 0%, rgb(0 0 0 / 0.5) 30%, transparent 70%)" },
  {
    value: "sides",
    label: "Sides",
    mask: "linear-gradient(to right, #000 0%, transparent 28%, transparent 72%, #000 100%), linear-gradient(to bottom, #000 0%, transparent 100%)",
    composite: "intersect",
  },
];

/** The fill of a layer with the "ink" tone. */
export const COLORS = [
  { value: "ink", label: "Ink", fill: "var(--color-base-content)" },
  { value: "primary", label: "Primary", fill: "var(--color-primary)" },
  { value: "secondary", label: "Secondary", fill: "var(--color-secondary)" },
  { value: "accent", label: "Accent", fill: "var(--color-accent)" },
  {
    value: "gradient",
    label: "Gradient",
    fill: "linear-gradient(110deg, var(--color-primary), var(--color-secondary), var(--color-accent))",
  },
];

export const TONE_FILLS = {
  success: "var(--color-success)",
  error: "var(--color-error)",
  warning: "var(--color-warning)",
};
