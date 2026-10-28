import { select } from "d3-selection";
import { area, curveBasis } from "d3-shape";
import { createNoise2D } from "simplex-noise";
import { frameLoop } from "./frame_loop.js";
import { starterPackColors } from "./palette.js";

const STEPS = 80;

export function mountStillWaves(stage) {
  const svg = select(stage).append("svg").attr("width", "100%").attr("height", "100%");
  const noise = createNoise2D();
  const layers = starterPackColors.map((color, i) => ({
    path: svg.append("path").style("fill", color).attr("fill-opacity", 0.22),
    height: 70 + i * 28,
    amplitude: 14 + i * 4,
    speed: 0.25 + i * 0.08,
    seed: i * 100,
  }));
  const waveArea = area().curve(curveBasis);
  const ripples = [];
  let clock = Math.random() * 100;
  let energy = 0;
  let drawnSize = null;

  const frames = frameLoop();
  frames.start((dt) => {
    const w = stage.clientWidth;
    const h = stage.clientHeight;
    energy = energy < 0.002 ? 0 : energy * Math.exp(-dt * 0.7);
    const resting = energy === 0 && ripples.length === 0;
    if (resting && drawnSize === `${w}x${h}`) return;
    drawnSize = `${w}x${h}`;
    clock += dt * energy * 1.2;
    waveArea.y0(h);
    for (let i = ripples.length - 1; i >= 0; i--) {
      ripples[i].age += dt;
      if (ripples[i].age > ripples[i].life) ripples.splice(i, 1);
    }
    for (const layer of layers) {
      /** @type {[number, number][]} */
      const points = [];
      for (let s = 0; s <= STEPS; s++) {
        const x = (w * s) / STEPS;
        let y = h - layer.height + noise(s * 0.08 + layer.seed, clock * layer.speed) * layer.amplitude;
        for (const r of ripples) {
          const spread = 60 + r.age * 420;
          const distance = Math.abs(x - r.x);
          const fade = Math.exp(-r.age * 1.6) * Math.exp(-((distance / spread) ** 2));
          y -= Math.sin(r.age * 9 - distance * 0.03) * r.strength * fade;
        }
        points.push([x, y]);
      }
      layer.path.attr("d", waveArea(points));
    }
  });

  // WebKit keeps the old background behind the waves until they redraw.
  const themeObserver = new MutationObserver(() => (drawnSize = null));
  themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });

  return {
    ripple(x, strength) {
      energy = Math.min(3, energy + strength / 45);
      ripples.push({ x, age: 0, life: 3, strength });
    },
    destroy() {
      themeObserver.disconnect();
      frames.stop();
      svg.remove();
    },
  };
}
