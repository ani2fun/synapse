/**
 * The code lab's glyphs. Stroke icons on a 24-unit box in `currentColor`, the same family the
 * other labs draw inline, so every control takes its colour from the button that holds it.
 */
import type { ComponentChildren } from "preact";

function Icon({ size = 15, children }: { size?: number; children: ComponentChildren }) {
  return (
    <svg
      viewBox="0 0 24 24"
      width={size}
      height={size}
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      stroke-linecap="round"
      stroke-linejoin="round"
      aria-hidden="true"
    >
      {children}
    </svg>
  );
}

export const PLAY = (
  <Icon size={13}>
    <path d="M6 4l14 8-14 8z" fill="currentColor"></path>
  </Icon>
);
export const RESET = (
  <Icon>
    <path d="M3 12a9 9 0 1 0 3-6.7"></path>
    <path d="M3 4v5h5"></path>
  </Icon>
);
export const FORMAT = (
  <Icon>
    <path d="M4 6h16M4 12h10M4 18h13"></path>
  </Icon>
);
export const COPY = (
  <Icon>
    <rect x="8" y="8" width="13" height="13" rx="2"></rect>
    <path d="M4 16V5a1 1 0 0 1 1-1h11"></path>
  </Icon>
);
export const TICK = (
  <Icon>
    <path d="M20 6 9 17l-5-5"></path>
  </Icon>
);
export const GEAR = (
  <Icon>
    <circle cx="12" cy="12" r="3"></circle>
    <path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"></path>
  </Icon>
);
export const EXPAND = (
  <Icon>
    <path d="M15 3h6v6M9 21H3v-6M21 3l-7 7M3 21l7-7"></path>
  </Icon>
);
export const SHRINK = (
  <Icon>
    <path d="M4 14h6v6M20 10h-6V4M14 10l7-7M3 21l7-7"></path>
  </Icon>
);
export const DOWNLOAD = (
  <Icon>
    <path d="M12 3v12M7 10l5 5 5-5M5 21h14"></path>
  </Icon>
);
export const UPLOAD = (
  <Icon>
    <path d="M12 21V9M7 14l5-5 5 5M5 3h14"></path>
  </Icon>
);
export const FOLDER = (
  <Icon>
    <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"></path>
  </Icon>
);
export const SAVE = (
  <Icon>
    <path d="M5 3h11l5 5v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z"></path>
    <path d="M7 3v5h8M7 21v-7h10v7"></path>
  </Icon>
);
export const SHARE = (
  <Icon>
    <circle cx="18" cy="5" r="3"></circle>
    <circle cx="6" cy="12" r="3"></circle>
    <circle cx="18" cy="19" r="3"></circle>
    <path d="m8.6 13.5 6.8 4M15.4 6.5l-6.8 4"></path>
  </Icon>
);
export const CLEAR = (
  <Icon>
    <path d="M3 6h18M8 6V4h8v2M6 6l1 14h10l1-14"></path>
  </Icon>
);
export const PLUS = (
  <Icon size={14}>
    <path d="M12 5v14M5 12h14"></path>
  </Icon>
);
export const CROSS = (
  <Icon size={12}>
    <path d="M18 6 6 18M6 6l12 12"></path>
  </Icon>
);
export const CHEVRON = (
  <Icon size={12}>
    <path d="m6 9 6 6 6-6"></path>
  </Icon>
);
export const TERMINAL = (
  <Icon size={26}>
    <path d="m4 17 6-5-6-5M12 19h8"></path>
  </Icon>
);
