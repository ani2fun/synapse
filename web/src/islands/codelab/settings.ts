/**
 * The code lab's editor look — what the settings popover edits and Monaco wears.
 *
 * Every field reads back through `parseSettings`, which clamps or replaces anything out of range,
 * so a hand-edited or stale stored value can only ever produce a look the popover could have set.
 * The font list is faces that fall back cleanly to the platform monospace: the page downloads no
 * extra font for a choice the reader may never make.
 */

export interface LabSettings {
  /** A Monaco theme id, or `default` — the Synapse pair following the site's light/dark mode. */
  readonly theme: string;
  readonly tabSize: number;
  readonly fontFamily: string;
  readonly fontSize: number;
}

export interface Choice {
  readonly value: string;
  readonly label: string;
}

export const THEMES: readonly Choice[] = [
  { value: "default", label: "Default (follows site)" },
  { value: "vs", label: "Light" },
  { value: "vs-dark", label: "Dark" },
  { value: "hc-black", label: "High contrast dark" },
  { value: "hc-light", label: "High contrast light" },
];

export const FONTS: readonly Choice[] = [
  { value: '"JetBrains Mono", ui-monospace, monospace', label: "JetBrains Mono" },
  { value: '"Fira Code", ui-monospace, monospace', label: "Fira Code" },
  { value: '"Source Code Pro", ui-monospace, monospace', label: "Source Code Pro" },
  { value: 'Menlo, Consolas, "DejaVu Sans Mono", ui-monospace, monospace', label: "System monospace" },
];

export const TAB_SIZES: readonly number[] = [2, 4, 8];
export const MIN_FONT_SIZE = 10;
export const MAX_FONT_SIZE = 24;

export const DEFAULT_SETTINGS: LabSettings = {
  theme: "default",
  tabSize: 4,
  fontFamily: FONTS[0]!.value,
  fontSize: 14,
};

export function clampFontSize(size: number): number {
  if (!Number.isFinite(size)) return DEFAULT_SETTINGS.fontSize;
  return Math.min(Math.max(Math.round(size), MIN_FONT_SIZE), MAX_FONT_SIZE);
}

/** A stored value back into a look, field by field: one bad field does not cost the others. */
export function parseSettings(raw: string | null): LabSettings {
  let value: Record<string, unknown> = {};
  try {
    const parsed: unknown = raw == null ? null : JSON.parse(raw);
    if (parsed != null && typeof parsed === "object") value = parsed as Record<string, unknown>;
  } catch {
    // An unreadable value is the defaults.
  }
  const pick = (choices: readonly Choice[], field: unknown, fallback: string) =>
    choices.some((choice) => choice.value === field) ? (field as string) : fallback;
  return {
    theme: pick(THEMES, value.theme, DEFAULT_SETTINGS.theme),
    tabSize: TAB_SIZES.includes(value.tabSize as number) ? (value.tabSize as number) : DEFAULT_SETTINGS.tabSize,
    fontFamily: pick(FONTS, value.fontFamily, DEFAULT_SETTINGS.fontFamily),
    fontSize: typeof value.fontSize === "number" ? clampFontSize(value.fontSize) : DEFAULT_SETTINGS.fontSize,
  };
}
