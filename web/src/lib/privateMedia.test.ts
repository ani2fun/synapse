import { describe, expect, it, vi } from "vitest";

describe("privateMedia", () => {
  it("resolves every path to itself until a resolver is installed", async () => {
    vi.resetModules();
    const media = await import("./privateMedia");
    expect(media.privateMediaActive()).toBe(false);
    await expect(media.resolveMedia("/media/book/a.png")).resolves.toBe("/media/book/a.png");
  });

  it("fetches a /media path once, however often it is asked for", async () => {
    vi.resetModules();
    const media = await import("./privateMedia");
    const fetchBlob = vi.fn(async (url: string) => `blob:${url}`);
    media.installPrivateMedia(fetchBlob);

    const [first, second] = await Promise.all([
      media.resolveMedia("/media/book/frame-1.png"),
      media.resolveMedia("/media/book/frame-1.png"),
    ]);

    expect(first).toBe("blob:/media/book/frame-1.png");
    expect(second).toBe(first);
    expect(fetchBlob).toHaveBeenCalledTimes(1);
  });

  it("leaves non-media URLs alone even when private", async () => {
    vi.resetModules();
    const media = await import("./privateMedia");
    const fetchBlob = vi.fn(async (url: string) => `blob:${url}`);
    media.installPrivateMedia(fetchBlob);

    await expect(media.resolveMedia("https://example.com/x.png")).resolves.toBe("https://example.com/x.png");
    expect(fetchBlob).not.toHaveBeenCalled();
  });

  it("falls back to the original path when the fetch is refused, so the image shows as broken", async () => {
    vi.resetModules();
    const media = await import("./privateMedia");
    media.installPrivateMedia(async () => {
      throw new Error("403");
    });

    await expect(media.resolveMedia("/media/book/gone.png")).resolves.toBe("/media/book/gone.png");
  });
});
