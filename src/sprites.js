// Hand-coded 8-bit crab sprite engine (Clawd-style: boxy flat-top body, wide-set
// square eyes, square claw nubs, three stubby legs). Every animation frame is the
// same parametric body in a different pose, produced by crab() — keeps frames
// consistent and the art in one place.
//
// Palette chars: . transparent  o dark accent  B body  s shadow  h highlight  e eye  w bubble

export const PALETTE = [
  "transparent", // 0 .
  "#8a4a38",     // 1 o  dark accent
  "#c97666",     // 2 B  body (Clawd terracotta)
  "#b06052",     // 3 s  body shadow
  "#db8a76",     // 4 h  highlight
  "#26150f",     // 5 e  eye
  "#f2e7dc",     // 6 w  speech-bubble "!"
];

const CHARS = { ".": 0, o: 1, B: 2, s: 3, h: 4, e: 5, w: 6 };

const W = 32;
const H = 32;

function compile(rows) {
  const grid = [];
  for (let y = 0; y < H; y++) {
    const line = rows[y] || "";
    const row = new Array(W).fill(0);
    for (let x = 0; x < W; x++) row[x] = CHARS[line[x]] ?? 0;
    grid.push(row);
  }
  return grid;
}

// ── Parametric crab pose ─────────────────────────────────────────────────────
// Geometry (rest): cap rows 10-11 cols 10-21 · body rows 12-21 cols 8-23 ·
// eyes 2×2 at cols 10/20 · nubs 4×4 at cols 4/24 · legs 2-wide at cols 10/15/20.
function crab(opts = {}) {
  const {
    dy = 0,          // whole-crab vertical offset (hop)
    eyes = "open",   // open | closed | up
    nubL = "side",   // side | mid | up   (claw nub height)
    nubR = "side",
    legs = "stand",  // stand | stepA | stepB (scurry phases)
    legShift = 0,    // legs x offset while running
    bubble = false,  // "!" attention bubble
  } = opts;

  const g = Array.from({ length: H }, () => Array(W).fill("."));
  const rect = (x, y, w, h, c) => {
    for (let yy = y + dy; yy < y + h + dy; yy++)
      for (let xx = x; xx < x + w; xx++)
        if (yy >= 0 && yy < H && xx >= 0 && xx < W) g[yy][xx] = c;
  };

  rect(10, 10, 12, 2, "B"); // cap
  rect(8, 12, 16, 10, "B"); // body
  rect(8, 21, 2, 1, "s");   // bottom corner shading
  rect(22, 21, 2, 1, "s");

  if (eyes === "closed") {
    rect(10, 13, 2, 1, "e");
    rect(20, 13, 2, 1, "e");
  } else {
    const ey = eyes === "up" ? 11 : 12;
    rect(10, ey, 2, 2, "e");
    rect(20, ey, 2, 2, "e");
  }

  const nubY = { side: 14, mid: 13, up: 11 };
  rect(4, nubY[nubL], 4, 4, "B");
  rect(24, nubY[nubR], 4, 4, "B");

  [10, 15, 20].forEach((x, i) => {
    const lifted =
      legs === "stepA" ? i === 1 : legs === "stepB" ? i !== 1 : false;
    rect(x + legShift, 22, 2, lifted ? 2 : 3, "B");
  });

  if (bubble) {
    rect(25, 3, 2, 5, "w"); // "!" bar
    rect(25, 9, 2, 2, "w"); // "!" dot
  }

  return g.map((r) => r.join(""));
}

const F = (opts, ms) => ({ cells: compile(crab(opts)), ms });

export const SPRITES = {
  // Idle: breathing bob + occasional blink.
  rest: { bob: true, frames: [F({}, 3200), F({ eyes: "closed" }, 130)] },
  // Thinking: eyes up, right claw taps.
  think: {
    bob: true,
    frames: [F({ eyes: "up" }, 420), F({ eyes: "up", nubR: "mid" }, 420)],
  },
  // Running a command: legs scurry.
  run: {
    frames: [
      F({ legs: "stepA", legShift: -1 }, 140),
      F({ legs: "stepB", legShift: 1 }, 140),
    ],
  },
  // Editing/writing: claws alternate like typing.
  type: { frames: [F({ nubL: "mid" }, 110), F({ nubR: "mid" }, 110)] },
  // Awaiting permission: claws up, "!" pulses.
  alert: {
    frames: [
      F({ nubL: "up", nubR: "up", bubble: true }, 500),
      F({ nubL: "up", nubR: "up" }, 350),
    ],
  },
  // Done: happy hop.
  celebrate: {
    frames: [F({ dy: -2, nubL: "up", nubR: "up" }, 170), F({}, 170)],
  },
};

// ── Renderer ───────────────────────────────────────────────────────────────
export class SpriteRenderer {
  constructor(canvas) {
    this.ctx = canvas.getContext("2d");
    this.ctx.imageSmoothingEnabled = false;
    this.anim = "rest";
    this.frame = 0;
    this.facing = 1; // 1 = right, -1 = flipped
    this._acc = 0;
    this._last = 0;
    this._t = 0;
    this._raf = null;
    this._loop = this._loop.bind(this);
  }

  play(anim) {
    if (!SPRITES[anim] || this.anim === anim) return;
    this.anim = anim;
    this.frame = 0;
    this._acc = 0;
  }

  setFacing(dir) {
    this.facing = dir === "left" ? -1 : 1;
  }

  start() {
    if (this._raf) return;
    this._last = performance.now();
    this._raf = requestAnimationFrame(this._loop);
  }

  _loop(now) {
    const dt = now - this._last;
    this._last = now;
    this._t += dt;
    const a = SPRITES[this.anim];
    this._acc += dt;
    if (this._acc >= a.frames[this.frame].ms) {
      this._acc = 0;
      this.frame = (this.frame + 1) % a.frames.length;
    }
    this._draw(a);
    this._raf = requestAnimationFrame(this._loop);
  }

  _draw(a) {
    const ctx = this.ctx;
    ctx.clearRect(0, 0, W, H);
    const bob = a.bob ? Math.round(Math.sin(this._t / 900)) : 0;
    const cells = a.frames[this.frame].cells;
    ctx.save();
    if (this.facing === -1) {
      ctx.translate(W, 0);
      ctx.scale(-1, 1);
    }
    for (let y = 0; y < H; y++) {
      for (let x = 0; x < W; x++) {
        const idx = cells[y][x];
        if (idx === 0) continue;
        ctx.fillStyle = PALETTE[idx];
        ctx.fillRect(x, y + bob, 1, 1);
      }
    }
    ctx.restore();
  }
}
