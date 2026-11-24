/**
 * The language picker on the toolbar: the tab's language as the trigger, every runnable language
 * in the panel, a tick on the current one. Choosing closes the menu; the page decides what a
 * switch does to the buffer (`tabs.setLanguage`).
 */
import { useState } from "preact/hooks";

import { CHEVRON, TICK } from "./icons";
import { Popover } from "./Popover";
import { LAB_LANGUAGES, languageInfo } from "./starters";

export function LanguageMenu({ value, onChange }: { value: string; onChange: (language: string) => void }) {
  const [open, setOpen] = useState(false);
  return (
    <Popover
      open={open}
      onOpenChange={setOpen}
      label={`Language: ${languageInfo(value).label}`}
      triggerClass="clab-lang"
      trigger={
        <>
          <span>{languageInfo(value).label}</span>
          {CHEVRON}
        </>
      }
    >
      <ul class="clab-menu" role="listbox" aria-label="Language">
        {LAB_LANGUAGES.map((language) => (
          <li>
            <button
              type="button"
              role="option"
              aria-selected={language.id === value}
              class={`clab-menu__item${language.id === value ? " clab-menu__item--on" : ""}`}
              onClick={() => {
                setOpen(false);
                if (language.id !== value) onChange(language.id);
              }}
            >
              <span>{language.label}</span>
              {language.id === value && TICK}
            </button>
          </li>
        ))}
      </ul>
    </Popover>
  );
}
