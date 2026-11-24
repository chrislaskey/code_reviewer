// The background pattern at the top of pages: lines bent by noise, like a
// topographic map, drawn as an SVG mask so any theme colour shows through.

import alea from "alea";
import { createNoise2D } from "simplex-noise";

/** A CSS mask image of flow lines covering `width` by `height`, scaled by `size`. */
export function flowLinesMaskImage(width, height, size) {
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
  const body = `<path d='${paths.join("")}' fill='none' stroke='#000' stroke-width='1'/>`;
  const svg = `<svg xmlns='http://www.w3.org/2000/svg' width='${width}' height='${height}' viewBox='0 0 ${width} ${height}'>${body}</svg>`;
  return `url("data:image/svg+xml,${encodeURIComponent(svg)}")`;
}
