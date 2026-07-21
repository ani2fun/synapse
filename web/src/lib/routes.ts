// The client app-map. URL ↔ Page parsing is pure — no DOM, no fetch.

/** Every page shape the parser recognises. */
export type Page =
  | { kind: "library" }
  | { kind: "lesson"; path: string[] }
  | { kind: "blog" }
  | { kind: "blogPost"; slug: string }
  | { kind: "account" }
  | { kind: "admin" }
  | { kind: "notFound"; raw: string };

/** Parse already-split, already-decoded URL path segments. */
export function pageFromSegments(segments: string[]): Page {
  const parts = segments.filter((segment) => segment !== "");
  if (parts.length === 0) return { kind: "library" };

  const [head, ...rest] = parts;
  if (head === "synapse" && rest.length > 0) return { kind: "lesson", path: rest };
  if (head === "blog" && rest.length === 0) return { kind: "blog" };
  if (head === "blog" && rest.length === 1) return { kind: "blogPost", slug: rest[0] };
  if (head === "account" && rest.length === 0) return { kind: "account" };
  if (head === "admin" && rest.length === 0) return { kind: "admin" };
  return { kind: "notFound", raw: parts.join("/") };
}

/** The canonical URL — directory-mirror for lessons (ADR-S010). */
export function pageUrl(page: Page): string {
  switch (page.kind) {
    case "library":
      return "/";
    case "lesson":
      return `/synapse/${page.path.join("/")}`;
    case "blog":
      return "/blog";
    case "blogPost":
      return `/blog/${page.slug}`;
    case "account":
      return "/account";
    case "admin":
      return "/admin";
    case "notFound":
      return `/${page.raw}`;
  }
}

/** A directory-mirror path string (`a/b/c`) → its segments, dropping empties. */
export function segmentsOf(path: string): string[] {
  return path.split("/").filter((segment) => segment !== "");
}
