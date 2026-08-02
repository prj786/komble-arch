// Generates placeholder PNG icons for Tauri with zero dependencies.
// Replace later with real art: put a 1024px icon.png in src-tauri/icons
// or run `npm run tauri icon path/to/icon.png`.
import { deflateSync } from "node:zlib";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const outDir = join(root, "src-tauri", "icons");
mkdirSync(outDir, { recursive: true });

const CRC_TABLE = (() => {
  const t = new Int32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c;
  }
  return t;
})();
const crc32 = (buf) => {
  let c = -1;
  for (const b of buf) c = CRC_TABLE[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ -1) >>> 0;
};
const chunk = (type, data) => {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, "ascii"), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([len, body, crc]);
};

function png(size, draw) {
  const raw = Buffer.alloc(size * (size * 4 + 1));
  for (let y = 0; y < size; y++) {
    const row = y * (size * 4 + 1);
    raw[row] = 0; // filter: none
    for (let x = 0; x < size; x++) {
      const [r, g, b, a] = draw(x, y, size);
      const o = row + 1 + x * 4;
      raw[o] = r; raw[o + 1] = g; raw[o + 2] = b; raw[o + 3] = a;
    }
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(size, 0);
  ihdr.writeUInt32BE(size, 4);
  ihdr[8] = 8; ihdr[9] = 6; // 8-bit RGBA
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", ihdr),
    chunk("IDAT", deflateSync(raw, { level: 9 })),
    chunk("IEND", Buffer.alloc(0))
  ]);
}

// Komble — the shepherd of your apps. The glyph is a kombali (კომბალი, the
// Georgian shepherd's staff): an ivory crook planted on a pasture hill, with
// one sheep of the flock, over a dusk-sky gradient frame. Not a solid colour —
// the frame IS the scene.
const IVORY = [246, 240, 229];
const HILL = [58, 74, 54];       // deep pasture green
const SHEEP = [250, 248, 242];
const SKY0 = [116, 70, 100];     // dusky plum (bottom-left)
const SKY1 = [240, 166, 92];     // warm amber (top-right)

// distance from point p to segment a→b (unit coords)
function segDist(px, py, ax, ay, bx, by) {
  const vx = bx - ax, vy = by - ay;
  const t = Math.max(0, Math.min(1, ((px - ax) * vx + (py - ay) * vy) / (vx * vx + vy * vy)));
  return Math.hypot(px - (ax + t * vx), py - (ay + t * vy));
}

function draw(x, y, s) {
  const u = (v) => v * s;
  // rounded rect mask (inset 4%, radius 22%)
  const i = u(0.04), r = u(0.22), lo = i + r, hi = s - i - r;
  const cx = Math.min(Math.max(x, lo), hi), cy = Math.min(Math.max(y, lo), hi);
  if (x < i || y < i || x >= s - i || y >= s - i || Math.hypot(x - cx, y - cy) > r)
    return [0, 0, 0, 0];

  const px = x / s, py = y / s;

  // ── the crook: shaft + curled head, drawn first so it stands over everything
  const W = 0.042; // half-thickness
  // shaft, slightly tilted like a planted staff
  const dShaft = segDist(px, py, 0.585, 0.30, 0.545, 0.84);
  // curl: circle around (0.505, 0.265), radius 0.105 — from the shaft top,
  // over the crown, hooking back down on the left
  const hcx = 0.505, hcy = 0.265, hr = 0.105;
  const ang = Math.atan2(py - hcy, px - hcx); // -PI..PI, 0 = right
  const onArc = ang < 0.45 || ang > 2.4; // top sweep, open at lower-left
  const dHook = Math.abs(Math.hypot(px - hcx, py - hcy) - hr);
  const inCrook = dShaft < W || (onArc && dHook < W);

  // ── pasture hill along the bottom
  const hillY = 0.80 + 0.35 * (px - 0.42) * (px - 0.42);
  const inHill = py > hillY;

  // ── one sheep of the flock, resting on the hill
  const inSheep =
    Math.hypot((px - 0.30) / 1.35, py - 0.745) < 0.052 || // body
    Math.hypot(px - 0.245, py - 0.72) < 0.03;             // head
  const inSheepLeg =
    py > 0.77 && py < 0.815 &&
    (Math.abs(px - 0.275) < 0.011 || Math.abs(px - 0.335) < 0.011);

  if (inCrook) return [...IVORY, 255];
  if (inSheep) return [...SHEEP, 255];
  if (inSheepLeg || inHill) return [...HILL, 255];

  // ── dusk sky: diagonal gradient, bottom-left plum → top-right amber
  const t = Math.max(0, Math.min(1, (px + (1 - py)) / 2));
  const mix = (a, b) => Math.round(a + (b - a) * t);
  return [mix(SKY0[0], SKY1[0]), mix(SKY0[1], SKY1[1]), mix(SKY0[2], SKY1[2]), 255];
}

for (const [name, size] of [
  ["32x32.png", 32],
  ["128x128.png", 128],
  ["128x128@2x.png", 256],
  ["icon.png", 512]
]) {
  writeFileSync(join(outDir, name), png(size, draw));
  console.log("wrote", join("src-tauri/icons", name));
}
