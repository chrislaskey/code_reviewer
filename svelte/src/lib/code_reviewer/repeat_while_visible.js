// Calls `callback` every `intervalMs` while the page is visible, and once
// each time it's shown again. Returns a function that stops it.
export function repeatWhileVisible(callback, intervalMs) {
  let interval;
  const start = () => (interval ??= setInterval(callback, intervalMs));
  const stop = () => {
    clearInterval(interval);
    interval = undefined;
  };
  const onVisibilityChange = () => {
    if (document.hidden) return stop();
    callback();
    start();
  };
  if (!document.hidden) start();
  document.addEventListener("visibilitychange", onVisibilityChange);
  return () => {
    stop();
    document.removeEventListener("visibilitychange", onVisibilityChange);
  };
}
