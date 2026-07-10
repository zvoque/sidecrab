// App entry. Instantiates the sprite renderer and starts the idle loop. The live
// Claude-state feed (Task 5) and interactions (Task 6) attach here later.
import { SpriteRenderer } from "./sprites.js";

const IN_TAURI = typeof window.__TAURI__ !== "undefined";

window.addEventListener("DOMContentLoaded", () => {
  const canvas = document.getElementById("crab");
  const renderer = new SpriteRenderer(canvas);
  renderer.play("rest");
  renderer.start();
  window.__crab = renderer; // handy for manual poking during dev
  if (IN_TAURI) console.log("Clawd Pet running in Tauri");
});
