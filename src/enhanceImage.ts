import { useEffect, useState } from "preact/hooks";

// Cover art from Kitsu/AniList is often smaller than the hero it fills, so
// it's upscaled to roughly screen size and unsharp-masked once, then cached
// as a blob URL. Anything that goes wrong (CORS-blocked fetch, decode
// failure, no canvas filter support) falls back to the original URL.

const TARGET_WIDTH = 1920;
const MAX_SCALE = 2;
const SHARPEN_AMOUNT = 0.6;
const BLUR_RADIUS_PX = 1.2;

const cache = new Map<string, Promise<string>>();

async function enhance(url: string): Promise<string> {
  console.debug("[enhanceImage] start", { url });
  const resp = await fetch(url, { mode: "cors" });
  if (!resp.ok) throw new Error(`image request failed: ${resp.status}`);
  const bitmap = await createImageBitmap(await resp.blob());

  const scale = Math.min(MAX_SCALE, Math.max(1, TARGET_WIDTH / bitmap.width));
  const width = Math.round(bitmap.width * scale);
  const height = Math.round(bitmap.height * scale);
  console.debug("[enhanceImage] scaling", { from: [bitmap.width, bitmap.height], to: [width, height] });

  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  if (!ctx) throw new Error("no 2d context");
  ctx.imageSmoothingEnabled = true;
  ctx.imageSmoothingQuality = "high";
  ctx.drawImage(bitmap, 0, 0, width, height);
  bitmap.close();

  // Unsharp mask: sharp = original + amount * (original - blurred).
  const blurCanvas = document.createElement("canvas");
  blurCanvas.width = width;
  blurCanvas.height = height;
  const blurCtx = blurCanvas.getContext("2d", { willReadFrequently: true });
  if (!blurCtx) throw new Error("no 2d context");
  blurCtx.filter = `blur(${BLUR_RADIUS_PX}px)`;
  blurCtx.drawImage(canvas, 0, 0);

  const image = ctx.getImageData(0, 0, width, height);
  const blurred = blurCtx.getImageData(0, 0, width, height).data;
  const px = image.data;
  for (let i = 0; i < px.length; i += 4) {
    px[i] += SHARPEN_AMOUNT * (px[i] - blurred[i]);
    px[i + 1] += SHARPEN_AMOUNT * (px[i + 1] - blurred[i + 1]);
    px[i + 2] += SHARPEN_AMOUNT * (px[i + 2] - blurred[i + 2]);
  }
  ctx.putImageData(image, 0, 0);

  const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, "image/jpeg", 0.92));
  if (!blob) throw new Error("canvas export failed");
  console.info("[enhanceImage] done", { url, bytes: blob.size });
  return URL.createObjectURL(blob);
}

export function enhanceImage(url: string): Promise<string> {
  let promise = cache.get(url);
  if (!promise) {
    promise = enhance(url).catch((err) => {
      console.warn("[enhanceImage] failed, using original", { url, err: String(err) });
      return url;
    });
    cache.set(url, promise);
  }
  return promise;
}

/** The sharpened/upscaled version of `url` once ready; the original until then. */
export function useEnhancedImage(url: string | null | undefined): string | undefined {
  const [enhanced, setEnhanced] = useState<{ source: string; url: string } | null>(null);
  useEffect(() => {
    if (!url) return;
    let cancelled = false;
    enhanceImage(url).then((result) => {
      if (!cancelled) setEnhanced({ source: url, url: result });
    });
    return () => {
      cancelled = true;
    };
  }, [url]);
  if (!url) return undefined;
  return enhanced?.source === url ? enhanced.url : url;
}
