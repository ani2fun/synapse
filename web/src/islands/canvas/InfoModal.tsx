/**
 * The "What is expected here" dialog behind every ℹ️ — the area's purpose, a checklist of what a
 * filled-in version contains, and a link to the source that says it at length.
 *
 * Escape and a scrim click both close, because a modal that traps a reader mid-thought is worse
 * than no guidance at all.
 */
import { useEffect } from "preact/hooks";

import { APPENDIX_PATH } from "./guidance";
import type { AreaGuidance } from "./guidance";

export function InfoModal({ info, onClose }: { info: AreaGuidance; onClose: () => void }) {
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div class="pcanvas__scrim" onClick={onClose}>
      <div
        class="pcanvas__modal"
        role="dialog"
        aria-modal="true"
        aria-label={`What is expected in ${info.title}`}
        onClick={(event) => event.stopPropagation()}
      >
        <div class="pcanvas__modal-head">
          <div>
            <p class="pcanvas__modal-eyebrow">What is expected here</p>
            <h2 class="pcanvas__modal-title">{info.title}</h2>
          </div>
          <button class="pcanvas__modal-close" type="button" aria-label="Close" onClick={onClose}>
            {"×"}
          </button>
        </div>
        <p class="pcanvas__modal-what">{info.what}</p>
        <p class="pcanvas__modal-label">Checklist</p>
        <ul class="pcanvas__modal-list">
          {info.bullets.map((bullet) => (
            <li key={bullet}>{bullet}</li>
          ))}
        </ul>
        <div class="pcanvas__modal-links">
          {/* The in-app long form leads: it covers this area at length, and it is the one that
              also carries what the outside write-ups leave out. The source is credited beside it
              rather than replaced — and it still works where the DSA book is not mounted. */}
          <a class="pcanvas__modal-link" href={`${APPENDIX_PATH}#${info.lessonHash}`}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20" />
              <path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z" />
            </svg>
            Read: {info.title} in depth
          </a>
          <a
            class="pcanvas__modal-link pcanvas__modal-link--source"
            href={info.href}
            target="_blank"
            rel="noopener noreferrer"
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6" />
              <path d="M15 3h6v6 M10 14L21 3" />
            </svg>
            {info.linkLabel}
          </a>
        </div>
      </div>
    </div>
  );
}
