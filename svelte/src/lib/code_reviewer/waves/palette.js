/** The StillWaves layers, back to front. The colours and opacities are set per theme in ../waves.css. */
export const waveLayers = [1, 2, 3].map((n) => ({ color: `var(--wave-color-${n})`, opacity: `var(--wave-opacity-${n})` }));
