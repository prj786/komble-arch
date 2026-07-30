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

// GNOME-blue rounded square with a white "download into tray" glyph.
const BG = [53, 132, 228], FG = [255, 255, 255];
function draw(x, y, s) {
  const u = (v) => v * s; // unit helper (fractions of size)
  // rounded rect (inset 4%, radius 22%)
  const i = u(0.04), r = u(0.22), lo = i + r, hi = s - i - r;
  const cx = Math.min(Math.max(x, lo), hi), cy = Math.min(Math.max(y, lo), hi);
  const d = Math.hypot(x - cx, y - cy);
  if (x < i || y < i || x >= s - i || y >= s - i || d > r) return [0, 0, 0, 0];
  // glyph: arrow shaft + head + tray bar
  const midX = s / 2;
  const inShaft = Math.abs(x - midX) < u(0.075) && y > u(0.22) && y < u(0.5);
  const headY = y - u(0.5);
  const inHead = headY >= 0 && headY < u(0.2) &&
    Math.abs(x - midX) < u(0.24) * (1 - headY / u(0.2));
  const inTray =
    (y > u(0.76) && y < u(0.84) && x > u(0.24) && x < s - u(0.24)) ||
    (x > u(0.24) && x < u(0.32) && y > u(0.62) && y < u(0.84)) ||
    (x > s - u(0.32) && x < s - u(0.24) && y > u(0.62) && y < u(0.84));
  return inShaft || inHead || inTray ? [...FG, 255] : [...BG, 255];
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
