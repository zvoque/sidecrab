// Hand-coded 8-bit crab sprite engine. Frames are authored as arrays of equal-length
// strings (one char per pixel) and compiled to palette-index grids at load. This keeps
// the art readable/editable inline instead of as opaque number matrices.
//
// Char map:  . transparent   o outline/dark   B body   s shadow   h highlight   e eye

export const PALETTE = [
  "transparent", // 0 .
  "#2a1608",     // 1 o  outline / legs
  "#d9772e",     // 2 B  shell body (warm orange)
  "#b25f22",     // 3 s  shell shadow
  "#eca05a",     // 4 h  highlight
  "#140d05",     // 5 e  eye
];

const CHARS = { ".": 0, o: 1, B: 2, s: 3, h: 4, e: 5 };

const W = 32;
const H = 32;

function compile(rows) {
  // Pad/validate to a W×H grid of palette indices.
  const grid = [];
  for (let y = 0; y < H; y++) {
    const line = rows[y] || "";
    const row = new Array(W).fill(0);
    for (let x = 0; x < W; x++) row[x] = CHARS[line[x]] ?? 0;
    grid.push(row);
  }
  return grid;
}

// ── Rest pose ────────────────────────────────────────────────────────────────
// A stout round-shelled crab facing the viewer: two eyes, two raised claws, legs.
const REST = [
  "................................",
  "................................",
  "................................",
  "................................",
  "................................",
  "................................",
  ".............oooooo.............",
  "...........oBBBBBBBBo...........",
  "..........oBBBBBBBBBBo..........",
  ".........oBBBBBBBBBBBBo.........",
  ".........oBBBBBBBBBBBBo.........",
  "........oBBBeeBBBBeeBBBo........",
  "........oBBBeeBBBBeeBBBo........",
  "..oooo..oBBBBBBBBBBBBBBo..oooo..",
  ".oBBBBo.oBBBBBBBBBBBBBBo.oBBBBo.",
  ".oBBBBBBoBBBBBBBBBBBBBBoBBBBBBo.",
  ".oBBBBBBoBBBBBBBBBBBBBBoBBBBBBo.",
  "...oo...oBBBBBBBBBBBBBBo...oo...",
  "........oBBBBBBBBBBBBBBo........",
  ".........oBBBBBBBBBBBBo.........",
  "..........oBBBBBBBBBBo..........",
  "...........oBBBBBBBBo...........",
  "............oooooooo............",
  "...........o..o..o..o...........",
  "................................",
  "................................",
  "................................",
  "................................",
  "................................",
  "................................",
  "................................",
  "................................",
];

// Blink: eyes closed — collapse the two eye rows to a single dark line.
const REST_BLINK = REST.map((r, y) => (y === 11 ? r.replace(/e/g, "B") : r));

export const SPRITES = {
  rest: {
    loop: true,
    bob: true, // gentle vertical breathing applied by the renderer
    frames: [
      { cells: compile(REST), ms: 3200 },
      { cells: compile(REST_BLINK), ms: 130 },
    ],
  },
};

// ── Renderer ───────────────────────────────────────────────────────────────
export class SpriteRenderer {
  constructor(canvas) {
    this.ctx = canvas.getContext("2d");
    this.ctx.imageSmoothingEnabled = false;
    this.anim = "rest";
    this.frame = 0;
    this.facing = 1; // 1 = right (natural), -1 = flipped
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
    const cur = a.frames[this.frame];
    if (this._acc >= cur.ms) {
      this._acc = 0;
      this.frame = (this.frame + 1) % a.frames.length;
    }
    this._draw(a);
    this._raf = requestAnimationFrame(this._loop);
  }

  _draw(a) {
    const ctx = this.ctx;
    ctx.clearRect(0, 0, W, H);
    const bob = a.bob ? Math.round(Math.sin(this._t / 900) * 1) : 0;
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
