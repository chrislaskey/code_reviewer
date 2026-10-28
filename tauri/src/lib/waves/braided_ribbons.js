import alea from "alea";
import { select } from "d3-selection";
import { curveBasis, line } from "d3-shape";
import { createNoise2D } from "simplex-noise";
import { dark, themeColors } from "./palette.js";

const STEPS = 90;
// Seeded, so every page draws the same braid.
const SEED = "code-reviewer";

export function mountBraidedRibbons(stage, { level = 0.5, amplitude = 36 } = {}) {
  const svg = select(stage).append("svg").attr("width", "100%").attr("height", "100%");
  const noise = createNoise2D(alea(SEED));
  const ribbons = themeColors.map((color, i) => ({
    path: svg
      .append("path")
      .style("stroke", color)
      .attr("fill", "none")
      .attr("stroke-linecap", "round")
      .attr("stroke-width", 2.5),
    seed: i * 40,
  }));
  const ribbonLine = line().curve(curveBasis);

  function draw() {
    const w = stage.clientWidth;
    const h = stage.clientHeight;
    const isDark = dark();
    svg.style("mix-blend-mode", isDark ? "screen" : "multiply");
    for (const ribbon of ribbons) {
      /** @type {[number, number][]} */
      const points = [];
      for (let s = 0; s <= STEPS; s++) {
        const x = (w * s) / STEPS;
        points.push([x, h * level + noise(s * 0.05 + ribbon.seed, 0) * amplitude]);
      }
      ribbon.path.attr("d", ribbonLine(points)).attr("stroke-opacity", isDark ? 0.55 : 0.5);
    }
  }

  draw();
  const resizeObserver = new ResizeObserver(draw);
  resizeObserver.observe(stage);
  const themeObserver = new MutationObserver(draw);
  themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });

  return {
    destroy() {
      resizeObserver.disconnect();
      themeObserver.disconnect();
      svg.remove();
    },
  };
}
