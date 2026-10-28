export function frameLoop() {
  let frame = null;
  let last = null;
  return {
    start(draw) {
      const tick = (now) => {
        // Capped, so a background tab coming back doesn't jump.
        const dt = last === null ? 0 : Math.min((now - last) / 1000, 0.1);
        last = now;
        draw(dt, now / 1000);
        frame = requestAnimationFrame(tick);
      };
      frame = requestAnimationFrame(tick);
    },
    stop() {
      if (frame !== null) cancelAnimationFrame(frame);
    },
  };
}
