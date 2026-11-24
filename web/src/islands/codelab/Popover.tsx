/**
 * The one popover both of the lab's menus use: a trigger, and a panel under it while open.
 *
 * Closing is the part worth sharing. A transparent scrim under the panel takes the outside click,
 * so a click that dismisses the menu never also lands on whatever sat beneath it (a Run button,
 * say); Escape closes and hands focus back to the trigger, so a keyboard reader is not left
 * focused on a node that no longer exists.
 */
import type { ComponentChildren } from "preact";
import { useEffect, useRef } from "preact/hooks";

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** The trigger's content and accessible name. */
  trigger: ComponentChildren;
  label: string;
  triggerClass: string;
  /** Which edge of the trigger the panel lines up with. */
  align?: "start" | "end";
  children: ComponentChildren;
}

export function Popover({ open, onOpenChange, trigger, label, triggerClass, align = "start", children }: Props) {
  const button = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.stopPropagation();
      onOpenChange(false);
      button.current?.focus();
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [open, onOpenChange]);

  return (
    <div class="clab-pop">
      <button
        ref={button}
        type="button"
        class={triggerClass}
        aria-label={label}
        title={label}
        aria-haspopup="true"
        aria-expanded={open}
        onClick={() => onOpenChange(!open)}
      >
        {trigger}
      </button>
      {open && (
        <>
          <div class="clab-pop__scrim" onClick={() => onOpenChange(false)}></div>
          <div class={`clab-pop__panel clab-pop__panel--${align}`} role="dialog" aria-label={label}>
            {children}
          </div>
        </>
      )}
    </div>
  );
}
