import { describe, expect, it } from "vitest";

import { expectedFills, foldAccents, grade, normalizeSpace } from "./exerciseMatch";

const BLANK = "_________________";

describe("expectedFills", () => {
  it("diffs a single blank against the answer line", () => {
    expect(
      expectedFills(
        `Tom ${BLANK} au tennis tous les mercredis. (jouer)`,
        "Tom jouait au tennis tous les mercredis. (jouer)",
        ["jouait"],
      ),
    ).toEqual(["jouait"]);
  });

  it("recovers a fill wider than the bold run (negation)", () => {
    expect(
      expectedFills(
        `L'électricité ${BLANK} à l'époque. (exister/négation)`,
        "L'électricité n'existait pas à l'époque. (exister/négation)",
        ["existait"],
      ),
    ).toEqual(["n'existait pas"]);
  });

  it("keeps the reflexive pronoun of a pronominal verb", () => {
    expect(
      expectedFills(
        `Nous ${BLANK} très souvent dans les bois. (se promener)`,
        "Nous nous promenions très souvent dans les bois. (se promener)",
        ["nous promenions"],
      ),
    ).toEqual(["nous promenions"]);
  });

  it("does not take a short word inside the fill for the trailing segment", () => {
    expect(expectedFills(`Je ${BLANK} à Londres. (partir)`, "Je suis parti à Londres. (partir)", [])).toEqual([
      "suis parti",
    ]);
  });

  it("fills two blanks in order", () => {
    expect(
      expectedFills(`Quand j'${BLANK} petit, je ${BLANK} au parc.`, "Quand j'étais petit, je jouais au parc.", []),
    ).toEqual(["étais", "jouais"]);
  });

  it("handles a blank at the very start", () => {
    expect(expectedFills(`${BLANK} au tennis.`, "Jouait au tennis.", [])).toEqual(["Jouait"]);
  });

  it("ignores a trailing hint the answer line leaves out", () => {
    expect(expectedFills(`Tu ${BLANK} une canne. (utiliser)`, "Tu utilisais une canne.", [])).toEqual(["utilisais"]);
  });

  it("treats curly and straight apostrophes and extra spaces as the same text", () => {
    expect(expectedFills(`L’enfant  ${BLANK}  tard.`, "L'enfant dormait tard.", [])).toEqual(["dormait"]);
  });

  it("falls back to the bold runs when the sentences do not line up", () => {
    expect(expectedFills(`Il ${BLANK} hier.`, "Hier, il est venu.", ["est venu"])).toEqual(["est venu"]);
  });

  it("returns null when nothing pairs up", () => {
    expect(expectedFills(`Il ${BLANK} hier.`, "Hier, il est venu.", [])).toBeNull();
    expect(expectedFills(`Il ${BLANK} et ${BLANK}.`, "Rien à voir.", ["un"])).toBeNull();
  });

  it("returns null for an item with no blank", () => {
    expect(expectedFills("567", "567 : cinq-cent-soixante-sept", [])).toBeNull();
  });

  it("does not count a short underscore run as a blank", () => {
    expect(expectedFills("snake__case", "snake__case", [])).toBeNull();
  });
});

describe("helpers", () => {
  it("normalizeSpace collapses whitespace and unifies apostrophes", () => {
    expect(normalizeSpace("  n’existait \n pas ")).toBe("n'existait pas");
  });

  it("foldAccents strips accents only", () => {
    expect(foldAccents("préparais à l'été, ça")).toBe("preparais a l'ete, ca");
  });
});

describe("grade", () => {
  it("accepts the exact answer", () => {
    expect(grade("jouait", "jouait")).toBe("right");
  });

  it("ignores case, spacing and apostrophe style", () => {
    expect(grade("Jouait ", "jouait")).toBe("right");
    expect(grade("JOUAIT", "Jouait")).toBe("right");
    expect(grade("n\u2019existait  pas", "n'existait pas")).toBe("right");
    expect(grade("n' existait pas", "n'existait pas")).toBe("right");
  });

  it("separates a wrong accent from a wrong answer", () => {
    expect(grade("preparais", "préparais")).toBe("accent");
    expect(grade("jouàit", "jouait")).toBe("accent");
    expect(grade("Étais", "étais")).toBe("right");
  });

  it("marks a different word or an empty input wrong", () => {
    expect(grade("jouais", "jouait")).toBe("wrong");
    expect(grade("", "jouait")).toBe("wrong");
    expect(grade("   ", "jouait")).toBe("wrong");
  });
});
