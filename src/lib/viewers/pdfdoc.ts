import { getDocument, GlobalWorkerOptions, type PDFDocumentProxy, type RenderTask, RenderingCancelledException, Util } from "pdfjs-dist";
import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";

GlobalWorkerOptions.workerSrc = workerUrl;

/** One run of text on a page, in PDF units: its transform, width and height. */
export interface TextRun {
  str: string;
  t: number[];
  w: number;
  h: number;
}

export interface PdfPageText {
  text: string;
  runs: TextRun[];
}

export interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Opens a PDF from its bytes. */
export async function openPdf(bytes: ArrayBuffer): Promise<PDFDocumentProxy> {
  return getDocument({ data: new Uint8Array(bytes) }).promise;
}

/**
 * Every page's text, reporting progress (pages read / total) as it goes; stops early,
 * with the pages read so far, once `wanted()` turns false.
 */
export async function readPages(doc: PDFDocumentProxy, progress: (done: number) => void, wanted: () => boolean): Promise<PdfPageText[]> {
  const out: PdfPageText[] = [];
  for (let i = 1; i <= doc.numPages && wanted(); i++) {
    const page = await doc.getPage(i);
    const content = await page.getTextContent();
    const runs: TextRun[] = [];
    for (const item of content.items) {
      if ("str" in item && item.str.trim() !== "") {
        runs.push({ str: item.str, t: item.transform, w: item.width, h: item.height });
      }
    }
    out.push({ text: runs.map((r) => r.str).join(" "), runs });
    page.cleanup();
    progress(i);
  }
  return out;
}

/** The render running on each canvas: pdf.js refuses a second one on the same canvas. */
const rendering = new WeakMap<HTMLCanvasElement, RenderTask>();

/**
 * Draws page `index` (0-based) into `canvas` fitted to `width` CSS px, and returns
 * where each text run sits on it, in CSS px; `null` when a later call drew over it first.
 */
export async function renderPage(doc: PDFDocumentProxy, index: number, canvas: HTMLCanvasElement, width: number, runs: TextRun[]) {
  rendering.get(canvas)?.cancel();
  const page = await doc.getPage(index + 1);
  rendering.get(canvas)?.cancel();
  const base = page.getViewport({ scale: 1 });
  const scale = width / base.width;
  const viewport = page.getViewport({ scale });
  const dpr = window.devicePixelRatio || 1;
  canvas.width = Math.round(viewport.width * dpr);
  canvas.height = Math.round(viewport.height * dpr);
  canvas.style.width = `${viewport.width}px`;
  canvas.style.height = `${viewport.height}px`;
  const task = page.render({ canvas, viewport, transform: dpr === 1 ? undefined : [dpr, 0, 0, dpr, 0, 0] });
  rendering.set(canvas, task);
  try {
    await task.promise;
  } catch (e) {
    if (e instanceof RenderingCancelledException) {
      return null;
    }
    throw e;
  } finally {
    if (rendering.get(canvas) === task) {
      rendering.delete(canvas);
    }
    page.cleanup();
  }
  const boxes: Box[] = runs.map((r) => {
    const m = Util.transform(viewport.transform, r.t) as number[];
    const h = Math.hypot(m[2], m[3]) || r.h * scale;
    return { x: m[4], y: m[5] - h, w: r.w * scale, h };
  });
  return { width: viewport.width, height: viewport.height, boxes };
}
