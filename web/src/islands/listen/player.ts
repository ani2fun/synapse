// ──────────────────────────────────────────────────────────────────
// THE PLAYER — one <audio>, one stream, a small bar at the foot of the page
// ──────────────────────────────────────────────────────────────────
// ONE media element for the page's whole life, and every listen is one continuous MP3 from the
// server. That is what keeps playing on a locked phone and what the OS hands its lock-screen
// controls to (Media Session). Speed is `playbackRate`: the browser stretches the audio it already
// has, so changing it costs no synthesis and no cache key.
//
// The bar owns its preferences (voice, speed) rather than the reader's Themes & Settings view:
// only a reader on the listen list ever sees a voice picker, so it lives where only they will.

import { ApiFailure, listenMarks, startListen } from "../../lib/api/client";
import type { ListenConfig } from "../../lib/api/client";
import * as log from "../../lib/log";
import * as storage from "../../lib/storage";

const RATES = ["1", "1.25", "1.5", "1.75", "2"] as const;

/** How often the bar asks how long the units synthesised so far play. */
const MARKS_EVERY_MS = 2000;

export interface Player {
  /** Read these units aloud, replacing whatever is playing. `onUnit` hears which unit is being
   *  spoken as playback moves, and `null` when it stops; `jump` is what "Jump to speech" calls. */
  play: (
    units: string[],
    label: string,
    onUnit: (index: number | null) => void,
    jump: () => void,
  ) => Promise<void>;
  /** Offer "Jump to speech" — true when what is being said has scrolled out of view. */
  setAway: (away: boolean) => void;
}

function select(label: string, options: readonly [string, string][], current: string): HTMLSelectElement {
  const el = document.createElement("select");
  el.className = "listen-bar__select";
  el.setAttribute("aria-label", label);
  for (const [value, text] of options) {
    const option = document.createElement("option");
    option.value = value;
    option.textContent = text;
    option.selected = value === current;
    el.append(option);
  }
  return el;
}

function button(className: string, label: string, text: string): HTMLButtonElement {
  const el = document.createElement("button");
  el.type = "button";
  el.className = className;
  el.setAttribute("aria-label", label);
  el.textContent = text;
  return el;
}

/** Keep units within the server's per-listen limit, dropping from the END — a listen that stops
 *  early is better than one that is refused outright. */
function withinLimit(units: string[], maxChars: number): string[] {
  let total = 0;
  const kept: string[] = [];
  for (const unit of units) {
    total += unit.length;
    if (total > maxChars) break;
    kept.push(unit);
  }
  return kept;
}

export function createPlayer(config: ListenConfig): Player {
  const voiceIds = config.voices.map((v) => v.id);
  const storedVoice = storage.get(storage.LISTEN_VOICE_KEY);
  let voice = storedVoice && voiceIds.includes(storedVoice) ? storedVoice : voiceIds[0];
  const storedRate = storage.get(storage.LISTEN_RATE_KEY);
  let rate = storedRate && (RATES as readonly string[]).includes(storedRate) ? storedRate : "1";

  const audio = document.createElement("audio");
  audio.preload = "none";

  const bar = document.createElement("div");
  bar.className = "listen-bar";
  bar.hidden = true;
  bar.setAttribute("role", "region");
  bar.setAttribute("aria-label", "Listening");
  bar.dataset.listenBar = "";
  const toggle = button("listen-bar__btn listen-bar__btn--primary", "Pause", "❚❚");
  const stop = button("listen-bar__btn", "Stop listening", "■");
  // Shown only while what is being said is off screen: the way back to the voice.
  const jumpTo = button("listen-bar__jump", "Jump to where the speech is", "↧ Jump to speech");
  jumpTo.hidden = true;
  jumpTo.dataset.listenJump = "";
  let onJump: () => void = () => undefined;
  jumpTo.addEventListener("click", () => onJump());
  const status = document.createElement("span");
  status.className = "listen-bar__status";
  status.setAttribute("aria-live", "polite");
  const voicePick = select(
    "Voice",
    config.voices.map((v) => [v.id, v.label]),
    voice,
  );
  const ratePick = select(
    "Speed",
    RATES.map((r) => [r, `${r}×`]),
    rate,
  );
  bar.append(toggle, status, jumpTo, ratePick, voicePick, stop, audio);
  document.body.append(bar);

  // ── follow-along: where each unit starts on the stream's clock ──
  // `starts[i]` is unit i's start; `starts.length - 1` units have a known duration. The stream
  // is the units end to end, so a unit starts where the durations before it add up to.
  let starts: number[] = [0];
  let total = 0;
  let marksPath = "";
  let marksTimer: number | undefined;
  let onUnit: (index: number | null) => void = () => undefined;
  const pollMarks = async (): Promise<void> => {
    window.clearTimeout(marksTimer);
    const known = starts.length - 1;
    if (!marksPath || known >= total) return;
    try {
      const marks = await listenMarks(marksPath, known);
      if (marks.from === starts.length - 1) {
        for (const seconds of marks.durations) starts.push(starts[starts.length - 1] + seconds);
      }
    } catch {
      log.debug("listen: marks unavailable — following pauses");
    }
    if (starts.length - 1 < total) marksTimer = window.setTimeout(() => void pollMarks(), MARKS_EVERY_MS);
  };
  /** The unit playing at `t`: the last one starting at or before it. Past the known durations it
   *  is the next unit, which is playing but whose length the server has not reported yet. */
  const unitAt = (t: number): number => {
    let index = 0;
    while (index + 1 < starts.length && starts[index + 1] <= t) index += 1;
    return Math.min(index, total - 1);
  };
  const stopFollowing = (): void => {
    window.clearTimeout(marksTimer);
    marksPath = "";
    onUnit(null);
    jumpTo.hidden = true;
  };

  const say = (text: string): void => {
    status.textContent = text;
  };
  const syncToggle = (): void => {
    const paused = audio.paused;
    toggle.textContent = paused ? "▶" : "❚❚";
    toggle.setAttribute("aria-label", paused ? "Resume" : "Pause");
  };

  toggle.addEventListener("click", () => {
    if (audio.paused) void audio.play();
    else audio.pause();
  });
  stop.addEventListener("click", () => {
    stopFollowing();
    audio.pause();
    audio.removeAttribute("src");
    audio.load(); // drops the connection, so the server stops synthesising
    bar.hidden = true;
    log.info("listen: stopped");
  });
  ratePick.addEventListener("change", () => {
    rate = ratePick.value;
    audio.playbackRate = Number(rate);
    storage.set(storage.LISTEN_RATE_KEY, rate);
    log.debug(`listen: speed → ${rate}×`);
  });
  voicePick.addEventListener("change", () => {
    voice = voicePick.value;
    storage.set(storage.LISTEN_VOICE_KEY, voice);
    // A stream already playing keeps its voice; the units are already being said.
    if (!audio.paused) say("The new voice starts with your next listen.");
    log.debug(`listen: voice → ${voice}`);
  });
  audio.addEventListener("play", syncToggle);
  audio.addEventListener("pause", syncToggle);
  audio.addEventListener("playing", () => say("Listening"));
  audio.addEventListener("waiting", () => say("Preparing the next part…"));
  audio.addEventListener("timeupdate", () => {
    if (marksPath) onUnit(unitAt(audio.currentTime));
  });
  audio.addEventListener("ended", () => {
    stopFollowing();
    say("Finished");
    syncToggle();
    log.info("listen: finished");
  });
  audio.addEventListener("error", () => {
    if (!audio.getAttribute("src")) return; // a deliberate stop, not a failure
    say("Playback stopped — the speech engine may be unavailable.");
    log.warn("listen: playback error");
  });

  if ("mediaSession" in navigator) {
    navigator.mediaSession.setActionHandler("play", () => void audio.play());
    navigator.mediaSession.setActionHandler("pause", () => audio.pause());
    navigator.mediaSession.setActionHandler("stop", () => stop.click());
  }

  return {
    setAway(away) {
      jumpTo.hidden = !away || !marksPath;
    },
    async play(allUnits, label, follower, jump) {
      const units = withinLimit(allUnits, config.maxRequestChars);
      if (units.length === 0) {
        log.debug("listen: nothing speakable in that range");
        return;
      }
      stopFollowing();
      onUnit = follower;
      onJump = jump;
      bar.hidden = false;
      say("Preparing…");
      // Claim the element inside the click, before the network round trip: iOS only lets a page
      // start audio from a user gesture, and an `await` would otherwise spend it.
      audio.play().catch(() => undefined);
      try {
        const ticket = await startListen({ voice, units });
        audio.src = ticket.url;
        starts = [0];
        total = ticket.units;
        marksPath = ticket.url.replace(/\.mp3$/, "/marks");
        onUnit(0);
        void pollMarks();
        audio.playbackRate = Number(rate);
        if ("mediaSession" in navigator) {
          navigator.mediaSession.metadata = new MediaMetadata({ title: label, artist: "Synapse" });
        }
        await audio.play();
        log.info(`listen: playing ${ticket.units} unit(s) in ${voice}`);
      } catch (error) {
        say(error instanceof ApiFailure ? error.message : "Could not start listening.");
        log.warn(`listen: could not start — ${String(error)}`);
      }
    },
  };
}
