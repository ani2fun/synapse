/**
 * `normaliseRepo` is the only logic in the content-repositories panel, and it exists because of
 * what people actually paste: the repository's page URL, not the `owner/name` the API wants.
 *
 * It deliberately does NOT validate. The server owns that — a client that decided what a
 * repository is would be a second authority to keep in step, and the one that matters is the one
 * writing to the registry.
 */
import { describe, expect, it } from "vitest";

import { normaliseRepo } from "./ContentSourcesSection";

describe("normaliseRepo", () => {
  it("passes owner/name through", () => {
    expect(normaliseRepo("ani2fun/java-guide")).toBe("ani2fun/java-guide");
  });

  it("accepts what the GitHub UI puts on the clipboard", () => {
    for (const input of [
      "https://github.com/ani2fun/java-guide",
      "http://github.com/ani2fun/java-guide",
      "https://www.github.com/ani2fun/java-guide",
      "github.com/ani2fun/java-guide",
      "https://github.com/ani2fun/java-guide.git",
      "https://github.com/ani2fun/java-guide/",
      "  https://github.com/ani2fun/java-guide  ",
    ]) {
      expect(normaliseRepo(input), input).toBe("ani2fun/java-guide");
    }
  });

  /// A link copied from a file view still names the repository in its first two segments.
  it("takes the repository out of a deep link", () => {
    expect(normaliseRepo("https://github.com/ani2fun/java-guide/tree/main/01-first-steps")).toBe(
      "ani2fun/java-guide",
    );
    expect(normaliseRepo("https://github.com/ani2fun/java-guide/blob/main/book.json")).toBe(
      "ani2fun/java-guide",
    );
  });

  it("is empty for nothing, so the caller can say what it needs", () => {
    expect(normaliseRepo("")).toBe("");
    expect(normaliseRepo("   ")).toBe("");
  });

  /// Not the client's call. Anything that is not a GitHub URL reaches the server unchanged and
  /// comes back a 400 quoting the rule — better than a guess here that disagrees with the registry.
  it("leaves anything it cannot recognise alone", () => {
    expect(normaliseRepo("java-guide")).toBe("java-guide");
  });

  /// The truncation is for GitHub deep links ONLY. Applied blindly it would turn a wrong paste
  /// into a well-formed owner/name the registry stores and the fetcher then cannot resolve — a
  /// registration that looks right and never syncs is worse than one that is refused.
  it("does not truncate a non-GitHub path into a plausible owner/name", () => {
    expect(normaliseRepo("gitlab.com/someone/thing")).toBe("gitlab.com/someone/thing");
    expect(normaliseRepo("ani2fun/java-guide/extra")).toBe("ani2fun/java-guide/extra");
  });
});
