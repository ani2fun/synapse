/**
 * The lab's two trips across the browser's edge: a buffer out as a file, a file in as text.
 *
 * Nothing is uploaded. "Open" reads the file in this tab with `File.text()` and the page puts it in
 * a new tab; "Download" builds a blob URL and clicks a detached anchor. The server never sees
 * either.
 */

/** Past this a file is not source code, and Monaco would stall tokenising it. */
export const MAX_OPEN_BYTES = 1_000_000;

export function downloadText(filename: string, text: string): void {
  const url = URL.createObjectURL(new Blob([text], { type: "text/plain;charset=utf-8" }));
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  link.click();
  // The click has started the download; the URL is not needed after this turn of the loop.
  setTimeout(() => URL.revokeObjectURL(url), 0);
}

/** Ask for one file and read it as text; null when the reader cancels. Throws on one too large. */
export function pickTextFile(accept?: string): Promise<{ name: string; text: string } | null> {
  return new Promise((resolve, reject) => {
    const input = document.createElement("input");
    input.type = "file";
    if (accept != null) input.accept = accept;
    input.addEventListener("change", () => {
      const file = input.files?.[0];
      if (file == null) return resolve(null);
      if (file.size > MAX_OPEN_BYTES) return reject(new Error(`${file.name} is over 1 MB`));
      file.text().then((text) => resolve({ name: file.name, text }), reject);
    });
    input.addEventListener("cancel", () => resolve(null));
    input.click();
  });
}

/** A filename safe to download as: the tab name, with what a filesystem refuses replaced. */
export function fileNameFor(name: string, extension: string): string {
  const base = name.trim().replace(/[\\/:*?"<>|]+/g, "-").replace(/\s+/g, "_") || "code";
  return `${base}.${extension}`;
}
