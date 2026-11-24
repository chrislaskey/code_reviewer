// Whether images that move, an animated GIF say, are shown moving. The
// choice is kept as the theme is, in localStorage, under `app:animations`:
// absent, it follows the OS setting, still when the person asked the
// system for reduced motion and moving otherwise; "on" or "off" once the
// person has chosen here, whatever the system says.

const ANIMATIONS_KEY = "app:animations";
const CHANGED_EVENT = "app:animations-changed";
const reducedMotion = () => matchMedia("(prefers-reduced-motion: reduce)");

export function animationsOn() {
  const chosen = localStorage.getItem(ANIMATIONS_KEY);
  if (chosen === "on") return true;
  if (chosen === "off") return false;
  return !reducedMotion().matches;
}

/** @param {boolean} on */
export function setAnimationsOn(on) {
  localStorage.setItem(ANIMATIONS_KEY, on ? "on" : "off");
  window.dispatchEvent(new Event(CHANGED_EVENT));
}

/**
 * Calls back when the choice changes: here, in another window, or in
 * the system's setting while it is followed.
 * @param {(on: boolean) => void} callback
 * @returns {() => void} stops listening
 */
export function onAnimationsChanged(callback) {
  const changed = () => callback(animationsOn());
  const media = reducedMotion();
  window.addEventListener(CHANGED_EVENT, changed);
  window.addEventListener("storage", changed);
  media.addEventListener("change", changed);
  return () => {
    window.removeEventListener(CHANGED_EVENT, changed);
    window.removeEventListener("storage", changed);
    media.removeEventListener("change", changed);
  };
}
