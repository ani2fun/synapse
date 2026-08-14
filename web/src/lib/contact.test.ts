// The address exists twice — here and in `server/src/submission/http/dto.rs`, because a 403
// reaches API consumers who never load this bundle. Two copies are only safe while both are
// asserted, so this file is one half of that pair and `submission/http/dto_tests.rs` is the other.
import { describe, expect, it } from "vitest";

import { CONTACT_EMAIL, EDIT_ACCESS_TEXT, SUBMIT_ACCESS_TEXT, accessRequest } from "./contact";

describe("how a reader is told to ask for access", () => {
  it("gives one address, matching the server's copy", () => {
    // Change this and `server/src/submission/http/dto.rs`'s CONTACT_EMAIL together — both tests
    // name the literal, so neither moves quietly.
    expect(CONTACT_EMAIL).toBe("synapse.kakde.eu@gmail.com");
  });

  it("says the same thing at both gates", () => {
    // The shape is the point: a reader who meets the submit gate and later the edit gate should
    // recognise the second as the same kind of answer.
    for (const text of [EDIT_ACCESS_TEXT, SUBMIT_ACCESS_TEXT]) {
      expect(text).toContain(CONTACT_EMAIL);
      expect(text).toContain("email your GitHub username");
      expect(text).toContain("access may or may not be granted");
      expect(text.endsWith("Thanks for understanding.")).toBe(true);
    }
  });

  it("names which grant it is talking about", () => {
    // …because they are separate grants, and a reader holding one may not hold the other.
    expect(EDIT_ACCESS_TEXT).toContain("content-editor list");
    expect(SUBMIT_ACCESS_TEXT).toContain("submit list");
    expect(EDIT_ACCESS_TEXT).not.toBe(SUBMIT_ACCESS_TEXT);
  });

  it("reads as one sentence for any grant", () => {
    expect(accessRequest("Doing the thing needs a grant.")).toBe(
      "Doing the thing needs a grant. To request one, email your GitHub username to " +
        `${CONTACT_EMAIL} — access may or may not be granted. Thanks for understanding.`,
    );
  });

  it("keeps the address in one piece, so a mailto can be built from it", () => {
    // EditorPage splits the sentence on the address to make it a link; that only works while the
    // address appears exactly once and unbroken.
    for (const text of [EDIT_ACCESS_TEXT, SUBMIT_ACCESS_TEXT]) {
      expect(text.split(CONTACT_EMAIL)).toHaveLength(2);
    }
  });
});
