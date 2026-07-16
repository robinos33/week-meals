// Génère les icônes PWA (PNG) de Week Meals sans dépendance : un « W » blanc
// sur fond vert potager « Cantine ». Encodeur PNG minimal (zlib + CRC32).
//
//   node scripts/generate-icons.mjs
//
// Produit public/icons/{icon-192,icon-512,maskable-512}.png et
// public/apple-touch-icon-180.png.

import { writeFileSync, mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { deflateSync } from "node:zlib";

const __dirname = dirname(fileURLToPath(import.meta.url));
const PUBLIC = resolve(__dirname, "../public");

const GREEN = [63, 125, 84]; // #3f7d54
const WHITE = [255, 255, 255];

// Segments du « W » en coordonnées normalisées (0..1).
const W_POINTS = [
  [0.22, 0.32],
  [0.36, 0.7],
  [0.5, 0.46],
  [0.64, 0.7],
  [0.78, 0.32],
];

function distToSegment(px, py, [ax, ay], [bx, by]) {
  const dx = bx - ax;
  const dy = by - ay;
  const len2 = dx * dx + dy * dy;
  let t = len2 === 0 ? 0 : ((px - ax) * dx + (py - ay) * dy) / len2;
  t = Math.max(0, Math.min(1, t));
  const cx = ax + t * dx;
  const cy = ay + t * dy;
  return Math.hypot(px - cx, py - cy);
}

/** Couverture (0..1) du « W » à un point normalisé, avec anti-crénelage doux. */
function markCoverage(nx, ny, size) {
  const half = 0.075; // demi-épaisseur du trait
  const aa = 1.2 / size; // largeur de transition ~ 1 px
  let min = Infinity;
  for (let i = 0; i < W_POINTS.length - 1; i++) {
    min = Math.min(min, distToSegment(nx, ny, W_POINTS[i], W_POINTS[i + 1]));
  }
  return clamp01((half - min) / aa + 0.5);
}

/** Couverture (0..1) du fond arrondi ; `radius` en fraction (0 = plein carré). */
function bgCoverage(nx, ny, radius, size) {
  if (radius <= 0) return 1;
  const aa = 1.2 / size;
  // Distance signée à un carré arrondi centré (coordonnées 0..1).
  const qx = Math.abs(nx - 0.5) - (0.5 - radius);
  const qy = Math.abs(ny - 0.5) - (0.5 - radius);
  const outside = Math.hypot(Math.max(qx, 0), Math.max(qy, 0));
  const dist = outside + Math.min(Math.max(qx, qy), 0) - radius;
  return clamp01(0.5 - dist / aa);
}

function clamp01(v) {
  return Math.max(0, Math.min(1, v));
}

function renderPng(size, { radius }) {
  const data = Buffer.alloc(size * size * 4);
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const nx = (x + 0.5) / size;
      const ny = (y + 0.5) / size;
      const bg = bgCoverage(nx, ny, radius, size);
      const mark = markCoverage(nx, ny, size);
      // Compose : fond vert (alpha = bg), puis « W » blanc par-dessus.
      const r = Math.round(GREEN[0] * (1 - mark) + WHITE[0] * mark);
      const g = Math.round(GREEN[1] * (1 - mark) + WHITE[1] * mark);
      const b = Math.round(GREEN[2] * (1 - mark) + WHITE[2] * mark);
      const a = Math.round(255 * bg);
      const o = (y * size + x) * 4;
      data[o] = r;
      data[o + 1] = g;
      data[o + 2] = b;
      data[o + 3] = a;
    }
  }
  return encodePng(size, size, data);
}

// --- Encodeur PNG minimal -------------------------------------------------

const CRC_TABLE = (() => {
  const table = new Int32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    table[n] = c;
  }
  return table;
})();

function crc32(buf) {
  let c = 0xffffffff;
  for (let i = 0; i < buf.length; i++) c = CRC_TABLE[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function chunk(type, body) {
  const typeBuf = Buffer.from(type, "ascii");
  const len = Buffer.alloc(4);
  len.writeUInt32BE(body.length, 0);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([typeBuf, body])), 0);
  return Buffer.concat([len, typeBuf, body, crc]);
}

function encodePng(width, height, rgba) {
  const sig = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 6; // color type RGBA
  // filter, compression, interlace = 0
  const raw = Buffer.alloc((width * 4 + 1) * height);
  for (let y = 0; y < height; y++) {
    raw[y * (width * 4 + 1)] = 0; // filtre None
    rgba.copy(raw, y * (width * 4 + 1) + 1, y * width * 4, (y + 1) * width * 4);
  }
  const idat = deflateSync(raw, { level: 9 });
  return Buffer.concat([
    sig,
    chunk("IHDR", ihdr),
    chunk("IDAT", idat),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

// --- Sortie ---------------------------------------------------------------

mkdirSync(resolve(PUBLIC, "icons"), { recursive: true });

const outputs = [
  ["icons/icon-192.png", 192, { radius: 0.2 }],
  ["icons/icon-512.png", 512, { radius: 0.2 }],
  ["icons/maskable-512.png", 512, { radius: 0 }], // plein bord pour le masque
  ["apple-touch-icon-180.png", 180, { radius: 0 }],
];

for (const [name, size, options] of outputs) {
  const png = renderPng(size, options);
  writeFileSync(resolve(PUBLIC, name), png);
  console.log(`écrit ${name} (${size}×${size}, ${png.length} o)`);
}
