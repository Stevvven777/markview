// Stdlib-only PNG decoder for screenshot assertions (`node:zlib` + Buffer).
// Supports the non-interlaced 8-bit truecolor (2) and RGBA (6) PNGs that
// Playwright screenshots produce.

import { inflateSync } from "node:zlib";

export function decodePng(buffer) {
  if (buffer.length < 8 || buffer.readUInt32BE(0) !== 0x89504e47) {
    throw new Error("screenshot is not a PNG");
  }
  let off = 8;
  let header = null;
  const idat = [];
  while (off + 8 <= buffer.length) {
    const len = buffer.readUInt32BE(off);
    const type = buffer.toString("latin1", off + 4, off + 8);
    const data = buffer.subarray(off + 8, off + 8 + len);
    if (type === "IHDR") {
      header = {
        width: data.readUInt32BE(0),
        height: data.readUInt32BE(4),
        depth: data[8],
        color: data[9],
        interlace: data[12],
      };
    } else if (type === "IDAT") {
      idat.push(data);
    } else if (type === "IEND") {
      break;
    }
    off += 12 + len;
  }
  const { width, height, depth, color, interlace } = header;
  if (depth !== 8 || (color !== 6 && color !== 2) || interlace !== 0) {
    throw new Error(`unsupported PNG: depth=${depth} color=${color} interlace=${interlace}`);
  }
  const channels = color === 6 ? 4 : 3;
  const raw = inflateSync(Buffer.concat(idat));
  const stride = width * channels;
  const pixels = Buffer.alloc(height * stride);
  let pos = 0;
  for (let y = 0; y < height; y++) {
    const filter = raw[pos++];
    const row = raw.subarray(pos, pos + stride);
    pos += stride;
    const cur = pixels.subarray(y * stride, (y + 1) * stride);
    const prev = y > 0 ? pixels.subarray((y - 1) * stride, y * stride) : null;
    for (let x = 0; x < stride; x++) {
      const a = x >= channels ? cur[x - channels] : 0;
      const b = prev ? prev[x] : 0;
      const c = x >= channels && prev ? prev[x - channels] : 0;
      let v = row[x];
      if (filter === 1) v += a;
      else if (filter === 2) v += b;
      else if (filter === 3) v += (a + b) >> 1;
      else if (filter === 4) {
        const p = a + b - c;
        const pa = Math.abs(p - a);
        const pb = Math.abs(p - b);
        const pc = Math.abs(p - c);
        v += pa <= pb && pa <= pc ? a : pb <= pc ? b : c;
      }
      cur[x] = v & 0xff;
    }
  }
  return { width, height, channels, data: pixels };
}

// Pixels that differ from `bg` beyond a small tolerance.
export function inkPixels(png, bg, tolerance = 16) {
  const { width, height, channels, data } = png;
  let count = 0;
  for (let i = 0; i < width * height; i++) {
    const o = i * channels;
    if (channels === 4 && data[o + 3] < 8) continue;
    if (
      Math.abs(data[o] - bg.r) > tolerance ||
      Math.abs(data[o + 1] - bg.g) > tolerance ||
      Math.abs(data[o + 2] - bg.b) > tolerance
    ) {
      count++;
    }
  }
  return count;
}
