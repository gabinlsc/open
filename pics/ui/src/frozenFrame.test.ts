import { describe, expect, it } from "vitest";
import { decodeFrame } from "./frozenFrame";

function encode(w: number, h: number, px: number[]): ArrayBuffer {
  const buf = new ArrayBuffer(8 + px.length);
  const v = new DataView(buf);
  v.setUint32(0, w, true);
  v.setUint32(4, h, true);
  new Uint8Array(buf, 8).set(px);
  return buf;
}

describe("image figée du canvas", () => {
  it("lit l'en-tête et les pixels", () => {
    const f = decodeFrame(encode(2, 1, [1, 2, 3, 4, 5, 6, 7, 8]))!;
    expect([f.width, f.height]).toEqual([2, 1]);
    expect([...f.data]).toEqual([1, 2, 3, 4, 5, 6, 7, 8]);
  });
  it("refuse une taille incohérente ou un tampon tronqué", () => {
    expect(decodeFrame(encode(3, 1, [1, 2, 3, 4]))).toBeNull();
    expect(decodeFrame(new ArrayBuffer(4))).toBeNull();
    expect(decodeFrame(encode(0, 0, []))).toBeNull();
  });
});
