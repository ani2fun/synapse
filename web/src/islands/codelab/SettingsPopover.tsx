/**
 * The editor settings: theme, tab size, font family, font size. Every change applies at once —
 * the page restyles Monaco in place and stores the look — so there is no Save button to forget.
 */
import { useState } from "preact/hooks";

import { GEAR } from "./icons";
import { Popover } from "./Popover";
import {
  DEFAULT_SETTINGS,
  FONTS,
  MAX_FONT_SIZE,
  MIN_FONT_SIZE,
  TAB_SIZES,
  THEMES,
  clampFontSize,
  type LabSettings,
} from "./settings";

interface Props {
  value: LabSettings;
  onChange: (settings: LabSettings) => void;
}

export function SettingsPopover({ value, onChange }: Props) {
  const [open, setOpen] = useState(false);
  const set = (change: Partial<LabSettings>) => onChange({ ...value, ...change });
  const pick = (event: Event) => (event.target as HTMLSelectElement).value;

  return (
    <Popover open={open} onOpenChange={setOpen} label="Editor settings" triggerClass="pane-hd__btn" align="end" trigger={GEAR}>
      <div class="clab-set">
        <p class="clab-set__title">Editor settings</p>
        <label class="clab-set__row">
          <span>Editor theme</span>
          <select class="clab-set__ctl" value={value.theme} onChange={(event) => set({ theme: pick(event) })}>
            {THEMES.map((theme) => (
              <option value={theme.value}>{theme.label}</option>
            ))}
          </select>
        </label>
        <label class="clab-set__row">
          <span>Tab size</span>
          <select
            class="clab-set__ctl"
            value={String(value.tabSize)}
            onChange={(event) => set({ tabSize: Number(pick(event)) })}
          >
            {TAB_SIZES.map((size) => (
              <option value={String(size)}>{size} spaces</option>
            ))}
          </select>
        </label>
        <label class="clab-set__row">
          <span>Font family</span>
          <select class="clab-set__ctl" value={value.fontFamily} onChange={(event) => set({ fontFamily: pick(event) })}>
            {FONTS.map((font) => (
              <option value={font.value}>{font.label}</option>
            ))}
          </select>
        </label>
        <label class="clab-set__row">
          <span>Font size</span>
          <span class="clab-set__size">
            <input
              class="clab-set__range"
              type="range"
              min={MIN_FONT_SIZE}
              max={MAX_FONT_SIZE}
              value={value.fontSize}
              onInput={(event) => set({ fontSize: clampFontSize(Number((event.target as HTMLInputElement).value)) })}
            />
            <output class="clab-set__px">{value.fontSize}px</output>
          </span>
        </label>
        <button type="button" class="clab-set__reset" onClick={() => onChange(DEFAULT_SETTINGS)}>
          Restore defaults
        </button>
      </div>
    </Popover>
  );
}
