// The saved session: when the token is kept, spent and dropped.

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AUTH_REFRESH_KEY } from "../../storage";
import { forgetSession, isRejected, refreshSession, restoreSession, saveSession, type RefreshClient } from "./sessionToken";

const REJECTED = { response: { status: 400 } };

/** A client whose refresh swaps in `next`, or throws `failure`, recording the token it spent. */
function client(start: string | undefined, next: string, failure?: unknown) {
  const spent: (string | undefined)[] = [];
  const fake: RefreshClient = {
    refreshToken: start,
    async updateToken() {
      spent.push(fake.refreshToken);
      if (failure) throw failure;
      fake.refreshToken = next;
      return true;
    },
  };
  return { fake, spent };
}

describe("sessionToken", () => {
  const saved = () => window.localStorage.getItem(AUTH_REFRESH_KEY);

  beforeEach(() => {
    const items = new Map<string, string>();
    vi.stubGlobal("navigator", {});
    vi.stubGlobal("window", {
      localStorage: {
        getItem: (key: string) => items.get(key) ?? null,
        setItem: (key: string, value: string) => void items.set(key, value),
        removeItem: (key: string) => void items.delete(key),
      },
    });
  });
  afterEach(() => vi.unstubAllGlobals());

  it("restoresNothingWhenNothingIsSaved", async () => {
    const { fake, spent } = client(undefined, "next");
    expect(await restoreSession(fake)).toBe(false);
    expect(spent).toEqual([]);
  });

  it("restoresFromTheSavedTokenAndKeepsItsReplacement", async () => {
    window.localStorage.setItem(AUTH_REFRESH_KEY, "saved");
    const { fake, spent } = client(undefined, "next");
    expect(await restoreSession(fake)).toBe(true);
    expect(spent).toEqual(["saved"]);
    expect(saved()).toBe("next");
  });

  it("dropsASavedTokenKeycloakRejects", async () => {
    window.localStorage.setItem(AUTH_REFRESH_KEY, "dead");
    const { fake } = client(undefined, "next", REJECTED);
    expect(await restoreSession(fake)).toBe(false);
    expect(saved()).toBeNull();
    expect(fake.refreshToken).toBeUndefined();
  });

  it("keepsTheSavedTokenThroughANetworkFailure", async () => {
    window.localStorage.setItem(AUTH_REFRESH_KEY, "saved");
    const { fake } = client(undefined, "next", new TypeError("offline"));
    expect(await restoreSession(fake)).toBe(false);
    expect(saved()).toBe("saved");
  });

  it("refreshSpendsTheTokenAnotherTabSaved", async () => {
    window.localStorage.setItem(AUTH_REFRESH_KEY, "from-other-tab");
    const { fake, spent } = client("mine", "next");
    await refreshSession(fake, 60);
    expect(spent).toEqual(["from-other-tab"]);
    expect(saved()).toBe("next");
  });

  it("refreshStillWorksWhenStorageHoldsNothing", async () => {
    const { fake, spent } = client("mine", "next");
    await refreshSession(fake, 60);
    expect(spent).toEqual(["mine"]);
  });

  it("refreshReportsARejectionAndDropsTheToken", async () => {
    window.localStorage.setItem(AUTH_REFRESH_KEY, "dead");
    const { fake } = client("dead", "next", REJECTED);
    await expect(refreshSession(fake, 60)).rejects.toBe(REJECTED);
    expect(saved()).toBeNull();
  });

  it("refreshesOneAtATimeUnderTheLock", async () => {
    const order: string[] = [];
    let tail = Promise.resolve<unknown>(undefined);
    vi.stubGlobal("navigator", {
      locks: {
        request: (_name: string, task: () => Promise<unknown>) => {
          const run = tail.then(task);
          tail = run.catch(() => undefined);
          return run;
        },
      },
    });
    window.localStorage.setItem(AUTH_REFRESH_KEY, "t0");
    const tab = (): RefreshClient => {
      const c: RefreshClient = {
        async updateToken() {
          order.push(c.refreshToken!);
          await Promise.resolve();
          c.refreshToken = `${c.refreshToken}+`;
          return true;
        },
      };
      return c;
    };
    await Promise.all([refreshSession(tab(), -1), refreshSession(tab(), -1)]);
    expect(order).toEqual(["t0", "t0+"]);
  });

  it("saveAndForget", () => {
    saveSession({ refreshToken: "kept", updateToken: async () => false });
    expect(saved()).toBe("kept");
    saveSession({ updateToken: async () => false });
    expect(saved()).toBe("kept");
    forgetSession();
    expect(saved()).toBeNull();
  });

  it("onlyA400CountsAsRejected", () => {
    expect(isRejected(REJECTED)).toBe(true);
    expect(isRejected({ response: { status: 503 } })).toBe(false);
    expect(isRejected(new TypeError("offline"))).toBe(false);
    expect(isRejected(null)).toBe(false);
  });
});
