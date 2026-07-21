/**
 * The auth boot island (oracle: `AuthStore::provide` + the header's `<AccountChip/>`), loaded
 * from Base.astro on EVERY page — the header is global, so check-sso runs everywhere. The boot
 * flow itself is plain TS in ./store; the only Preact here is the header chip (menu state).
 * keycloak-js stays a lazy chunk (the loader dynamic-imports it) — this entry is small.
 */
import { render, h } from "preact";

import * as log from "../../lib/log";
import { boot } from "./store";
import { Chip } from "./Chip";

const host = document.querySelector<HTMLElement>("[data-account-chip]");
if (host) {
  host.replaceChildren();
  render(h(Chip, {}), host);
  log.debug("auth: header chip mounted");
}

void boot();
