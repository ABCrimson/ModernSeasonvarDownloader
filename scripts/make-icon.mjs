import { writeFileSync } from 'node:fs'
import { deflateSync } from 'node:zlib'

const S = 1024
const px = Buffer.alloc(S * S * 4)
const bg = [37, 38, 48],
  gold = [226, 186, 94],
  ink = [30, 26, 16]
const inRounded = (x, y, r) => {
  const cx = Math.min(Math.max(x, r), S - r),
    cy = Math.min(Math.max(y, r), S - r)
  return (x - cx) ** 2 + (y - cy) ** 2 <= r * r
}
for (let y = 0; y < S; y++)
  for (let x = 0; x < S; x++) {
    const i = (y * S + x) * 4
    let c = null
    if (inRounded(x, y, 180)) c = bg
    const dx = x - 512,
      dy = y - 512
    if (c && dx * dx + dy * dy <= 330 * 330) c = gold
    const shaft = Math.abs(dx) <= 70 && dy >= -260 && dy <= 60
    const head = dy > 40 && dy <= 280 && Math.abs(dx) <= 280 - (dy - 40)
    if (c && (shaft || head)) c = ink
    if (c) {
      px[i] = c[0]
      px[i + 1] = c[1]
      px[i + 2] = c[2]
      px[i + 3] = 255
    }
  }
const raw = Buffer.alloc((S * 4 + 1) * S)
for (let y = 0; y < S; y++) {
  raw[y * (S * 4 + 1)] = 0
  px.copy(raw, y * (S * 4 + 1) + 1, y * S * 4, (y + 1) * S * 4)
}
const crcTable = Array.from({ length: 256 }, (_, n) => {
  let c = n
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1
  return c >>> 0
})
const crc32 = (b) => {
  let c = 0xffffffff
  for (const x of b) c = crcTable[(c ^ x) & 0xff] ^ (c >>> 8)
  return (c ^ 0xffffffff) >>> 0
}
const chunk = (type, data) => {
  const len = Buffer.alloc(4)
  len.writeUInt32BE(data.length)
  const td = Buffer.concat([Buffer.from(type, 'ascii'), data])
  const crc = Buffer.alloc(4)
  crc.writeUInt32BE(crc32(td))
  return Buffer.concat([len, td, crc])
}
const ihdr = Buffer.alloc(13)
ihdr.writeUInt32BE(S, 0)
ihdr.writeUInt32BE(S, 4)
ihdr[8] = 8
ihdr[9] = 6
ihdr[10] = 0
ihdr[11] = 0
ihdr[12] = 0
const png = Buffer.concat([
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  chunk('IHDR', ihdr),
  chunk('IDAT', deflateSync(raw)),
  chunk('IEND', Buffer.alloc(0)),
])
writeFileSync(process.argv[2] ?? 'apps/desktop/src-tauri/app-icon.png', png)
// biome-ignore lint/suspicious/noConsole: CLI script reports its result on stdout
console.log('wrote icon')
