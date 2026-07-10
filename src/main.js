// Task 0 placeholder renderer. Proves the window + canvas pipeline works before the
// real sprite engine (Task 1) replaces this. Runs both inside Tauri and in a plain
// browser (for fast visual iteration), so it must not assume window.__TAURI__ exists.

const IN_TAURI = typeof window.__TAURI__ !== "undefined";

function drawPlaceholder(ctx) {
  ctx.clearRect(0, 0, 24, 24);
  // Body
  ctx.fillStyle = "#d9772e";
  ctx.fillRect(5, 8, 14, 9);
  ctx.fillRect(7, 6, 10, 2);
  // Claws
  ctx.fillRect(2, 10, 3, 4);
  ctx.fillRect(19, 10, 3, 4);
  // Eyes
  ctx.fillStyle = "#1a1208";
  ctx.fillRect(9, 9, 2, 2);
  ctx.fillRect(13, 9, 2, 2);
}

window.addEventListener("DOMContentLoaded", () => {
  const ctx = document.getElementById("crab").getContext("2d");
  drawPlaceholder(ctx);
  if (IN_TAURI) console.log("Clawd Pet running in Tauri");
});
