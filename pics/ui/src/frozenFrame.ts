/**
 * Image figée du canvas (voir `crates/app/src/frame.rs`) : largeur et hauteur en u32 petit-boutiste, puis RGBA.
 * Affichée à la place de la fenêtre native pendant qu'un menu ou une boîte de dialogue est ouvert.
 */
export interface Frame {
  width: number;
  height: number;
  data: Uint8ClampedArray<ArrayBuffer>;
}

export function decodeFrame(buf: ArrayBuffer): Frame | null {
  if (buf.byteLength < 8) return null;
  const view = new DataView(buf);
  const width = view.getUint32(0, true);
  const height = view.getUint32(4, true);
  if (width === 0 || height === 0 || buf.byteLength !== 8 + width * height * 4) return null;
  return { width, height, data: new Uint8ClampedArray(buf, 8) };
}
