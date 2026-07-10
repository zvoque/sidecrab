// Clawd renderer: draws the official 20-frame walk cycle (frames.js) on a canvas.
// Anim = a sequence of {i: frameIndex, ms, dy?, blink?, bubble?} steps. Facing flips
// horizontally. No ambient bobbing — idle is still; micro-life comes from the idle
// scheduler in state-machine.js picking occasional one-shot anims.
import { FRAME_W, FRAME_H, WALK_PNGS } from "./frames.js";

// Canvas is taller than a frame so the "!" bubble has headroom above the crab.
export const CANVAS_W = FRAME_W;
export const CANVAS_H = 48;
const CRAB_Y = CANVAS_H - FRAME_H; // crab sits at the bottom

// Animation definitions. Frames 0-3 of the cycle are the neutral stand; the rest
// walk with the sideways wobble.
export const SPRITES = {
  // Idle: dead still on the neutral pose (micro-life is scheduled separately).
  rest: { loop: true, steps: [{ i: 0, ms: 60000 }] },
  // Micro-idle one-shots (scheduler picks one every so often):
  blink: { loop: false, steps: [{ i: 0, ms: 160, blink: true }, { i: 0, ms: 120 }, { i: 0, ms: 140, blink: true }] },
  shuffle: {
    loop: false,
    steps: [ { i: 5, ms: 260 }, { i: 6, ms: 260 }, { i: 5, ms: 260 }, { i: 0, ms: 120 } ],
  },
  // Morning stretch: rises on extended legs, arms up-out, eyes shut (frames 22/23).
  stretch: {
    loop: false,
    steps: [ { i: 22, ms: 260 }, { i: 23, ms: 850 }, { i: 22, ms: 220 }, { i: 0, ms: 140 } ],
  },
  peek: {
    loop: false,
    steps: [ { i: 8, ms: 500 }, { i: 0, ms: 150 }, { i: 16, ms: 500 }, { i: 0, ms: 120 } ],
  },
  // Official walk cycle — frames 0-4 are the flat standing pose and would hiccup
  // the loop, so the cycle uses only the true walking frames.
  walk: {
    loop: true,
    steps: Array.from({ length: 15 }, (_, k) => ({ i: k + 5, ms: 70 })),
  },
  // Thinking: slow pensive shuffle.
  think: { loop: true, steps: [ { i: 4, ms: 500 }, { i: 5, ms: 500 }, { i: 4, ms: 500 }, { i: 0, ms: 700 } ] },
  // Working (any tool): side profile at his laptop, claw tapping, screen flicker.
  work: { loop: true, steps: [ { i: 24, ms: 260 }, { i: 25, ms: 260 } ] },
  // Panic: full-tilt scramble — used while being carried (drag) and cursor-chasing.
  panic: {
    loop: true,
    steps: Array.from({ length: 15 }, (_, k) => ({ i: k + 5, ms: 45 })),
  },
  // Mad glare: narrowed eyes, dead still (pre/post cursor-chase).
  glare: { loop: true, steps: [{ i: 0, ms: 60000, squint: true }] },
  // Hovered: crouch down (legs sink into the ground) and squint contentedly.
  hover: { loop: true, steps: [{ i: 0, ms: 60000, dy: 3, squint: true }] },
  // Asleep: eyes shut, gentle Z's drifting up (long-idle state).
  sleep: {
    loop: true,
    steps: [
      { i: 0, ms: 900, blink: true, dy: 2, zzz: 0 },
      { i: 0, ms: 900, blink: true, dy: 2, zzz: 1 },
    ],
  },
  // Glance around: the pupils actually move (micro-idle).
  look: {
    loop: false,
    steps: [
      { i: 0, ms: 550, eyesDx: -2 },
      { i: 0, ms: 200 },
      { i: 0, ms: 550, eyesDx: 2 },
      { i: 0, ms: 150 },
    ],
  },
  // Wave: raised claw with open pincer, wiggling (frames 20/21).
  wave: {
    loop: false,
    steps: [
      { i: 20, ms: 180 }, { i: 21, ms: 180 },
      { i: 20, ms: 180 }, { i: 21, ms: 180 },
      { i: 20, ms: 200 }, { i: 0, ms: 120 },
    ],
  },
  // Awaiting permission: still, urgent "!" bubble pulsing.
  alert: { loop: true, steps: [ { i: 0, ms: 550, bubble: true }, { i: 0, ms: 350 } ] },
  // Done: happy double hop.
  celebrate: {
    loop: true,
    steps: [ { i: 0, ms: 130, dy: -4 }, { i: 0, ms: 130 }, { i: 5, ms: 130, dy: -4 }, { i: 5, ms: 130 } ],
  },
};

const BUBBLE = "#f2e7dc";

export class SpriteRenderer {
  constructor(canvas) {
    this.canvas = canvas;
    this.ctx = canvas.getContext("2d");
    this._fit();
    window.addEventListener("resize", () => this._fit());
    this.anim = "rest";
    this.step = 0;
    this.facing = 1; // 1 = natural, -1 = flipped
    this._acc = 0;
    this._last = 0;
    this._raf = null;
    this._onFinish = null; // one-shot completion callback
    this._loop = this._loop.bind(this);

    this.images = WALK_PNGS.map((src) => {
      const img = new Image();
      img.src = src;
      return img;
    });
    this._eyeMask = null; // computed lazily once frame 0 loads
    this._boundsCache = {};
  }

  /// Play an animation. For non-looping (micro-idle) anims, onFinish fires once done.
  play(anim, onFinish = null) {
    if (!SPRITES[anim]) return;
    if (this.anim === anim && SPRITES[anim].loop) return;
    this.anim = anim;
    this.step = 0;
    this._acc = 0;
    this._onFinish = onFinish;
  }

  setFacing(dir) {
    this.facing = dir === "left" ? -1 : 1;
  }

  /// Match the backing store to physical pixels (window size × devicePixelRatio)
  /// so nothing is resampled by CSS; drawing scales logical→physical with
  /// smoothing off for hard pixel edges.
  _fit() {
    const dpr = window.devicePixelRatio || 1;
    const w = Math.max(1, Math.round(this.canvas.clientWidth * dpr));
    const h = Math.max(1, Math.round(this.canvas.clientHeight * dpr));
    if (this.canvas.width !== w || this.canvas.height !== h) {
      this.canvas.width = w;
      this.canvas.height = h;
    }
    this._scale = Math.min(w / CANVAS_W, h / CANVAS_H);
    this._ox = (w - CANVAS_W * this._scale) / 2;
    this._oy = (h - CANVAS_H * this._scale) / 2;
  }

  start() {
    if (this._raf) return;
    this._last = performance.now();
    this._raf = requestAnimationFrame(this._loop);
  }

  /// Dark opaque pixels of frame 0 = the eyes, including the darker halo around
  /// them (anti-aliasing from the source GIF) — missing the halo leaves an
  /// "eyelid" outline when the core is filled. Each pixel remembers the first
  /// clean body color below it so blink/squint can close the eye seamlessly.
  _computeEyeMask() {
    const img = this.images[0];
    if (!img.complete || img.naturalWidth === 0) return null;
    const off = new OffscreenCanvas(FRAME_W, FRAME_H);
    const ctx = off.getContext("2d");
    ctx.drawImage(img, 0, 0);
    const d = ctx.getImageData(0, 0, FRAME_W, FRAME_H).data;
    const isDark = (x, y) => {
      if (x < 0 || y < 0 || x >= FRAME_W || y >= FRAME_H) return false;
      const o = (y * FRAME_W + x) * 4;
      if (d[o + 3] < 150) return false;
      return 0.299 * d[o] + 0.587 * d[o + 1] + 0.114 * d[o + 2] <= 120;
    };
    const mask = [];
    const bottoms = new Map(); // x -> lowest eye y in that column
    for (let y = 0; y < FRAME_H; y++) {
      for (let x = 0; x < FRAME_W; x++) {
        if (!isDark(x, y)) continue;
        // first clean body pixel below (skip the rest of the eye/halo)
        let sy = y + 1;
        while (sy < FRAME_H && isDark(x, sy)) sy++;
        const s = (Math.min(sy, FRAME_H - 1) * FRAME_W + x) * 4;
        mask.push({ x, y, fill: `rgb(${d[s]},${d[s + 1]},${d[s + 2]})` });
        if ((bottoms.get(x) ?? -1) < y) bottoms.set(x, y);
      }
    }
    this._eyeBottoms = new Set([...bottoms].map(([x, y]) => `${x},${y}`));
    return mask;
  }

  _loop(now) {
    const dt = now - this._last;
    this._last = now;
    const a = SPRITES[this.anim];
    this._acc += dt;
    if (this._acc >= a.steps[this.step].ms) {
      this._acc = 0;
      if (this.step + 1 >= a.steps.length && !a.loop) {
        const cb = this._onFinish;
        this._onFinish = null;
        this.play("rest");
        if (cb) cb();
      } else {
        this.step = (this.step + 1) % a.steps.length;
      }
    }
    this._draw();
    this._raf = requestAnimationFrame(this._loop);
  }

  _draw() {
    const ctx = this.ctx;
    const s = SPRITES[this.anim].steps[this.step];
    const img = this.images[s.i];
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);
    if (!img.complete || img.naturalWidth === 0) return;
    // Logical (51×48) space, scaled to physical pixels; nearest-neighbor.
    ctx.setTransform(this._scale, 0, 0, this._scale, this._ox, this._oy);
    ctx.imageSmoothingEnabled = false;
    ctx.save();
    if (this.facing === -1) {
      ctx.translate(CANVAS_W, 0);
      ctx.scale(-1, 1);
    }
    const y = CRAB_Y + (s.dy || 0);
    ctx.drawImage(img, 0, y);
    if (s.blink || s.squint) {
      if (!this._eyeMask) this._eyeMask = this._computeEyeMask();
      for (const p of this._eyeMask || []) {
        // Squint keeps only the lowest eye pixel per column (flat closed line,
        // no leftover outline above); blink closes everything.
        if (s.squint && !s.blink && this._eyeBottoms?.has(`${p.x},${p.y}`)) continue;
        ctx.fillStyle = p.fill;
        ctx.fillRect(p.x, p.y + y, 1, 1);
      }
    }
    if (s.eyesDx) {
      // Glance: erase the pupils, redraw them shifted (real eye movement).
      if (!this._eyeMask) this._eyeMask = this._computeEyeMask();
      for (const p of this._eyeMask || []) {
        ctx.fillStyle = p.fill;
        ctx.fillRect(p.x, p.y + y, 1, 1);
      }
      ctx.fillStyle = "#000";
      for (const p of this._eyeMask || []) {
        ctx.fillRect(p.x + s.eyesDx, p.y + y, 1, 1);
      }
    }
    ctx.restore();
    if (s.zzz !== undefined) this._drawZzz(s.zzz);
    if (s.bubble) {
      // "!" above the crab, upper-right; drawn unflipped so it always reads.
      ctx.fillStyle = BUBBLE;
      ctx.fillRect(CANVAS_W - 12, 0, 3, 7);
      ctx.fillRect(CANVAS_W - 12, 9, 3, 3);
    }
  }

  /// Sleep Z's drifting up-right of the head; `phase` alternates the drift.
  _drawZzz(phase) {
    const ctx = this.ctx;
    ctx.fillStyle = BUBBLE;
    const drawZ = (x, y, s) => {
      ctx.fillRect(x, y, s, 1);             // top bar
      for (let k = 0; k < s - 2; k++) ctx.fillRect(x + s - 2 - k, y + 1 + k, 1, 1); // diagonal
      ctx.fillRect(x, y + s - 1, s, 1);     // bottom bar
    };
    if (phase === 0) {
      drawZ(40, 6, 4);
      drawZ(45, 1, 5);
    } else {
      drawZ(41, 4, 4);
      drawZ(46, 0, 5);
    }
  }

  /// Opaque bounding box (fractions of canvas) for the click-through hit rect.
  bounds() {
    // The crab body fills most of the frame; a fixed rect over the crab area is
    // accurate for this boxy sprite and avoids per-frame pixel scans.
    return { x0: 0.04, y0: CRAB_Y / CANVAS_H, x1: 0.96, y1: 1.0 };
  }
}
