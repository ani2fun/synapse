/**
 * The canvas itself: eight areas in the shape the method prescribes — Problem across the top,
 * then Constraints beside Maintenance, then Inputs · Return · Error/N-A, then Ideas, then Tests.
 * The order is the order you fill them in, and the widths say which ones deserve room.
 *
 * The areas carry NO placeholder text. What belongs in each is the ℹ️'s job, said once and at
 * length; a hint repeated inside the field reads as content the reader has to clear, and an empty
 * canvas full of grey sentences looks like a filled one. The only survivors are the T and S boxes,
 * where the hint is a FORMAT ("O(n²)") rather than a restatement of the label — nothing else tells
 * the reader that Big-O is what goes there.
 *
 * The textareas are UNCONTROLLED (`defaultValue` + `onInput`). A controlled textarea re-renders
 * the whole canvas on every keystroke, and this pane holds nine of them; the buffer instead lives
 * in the parent's mutable body object and the parent re-renders only when the derived readout
 * actually changes (an area crossing empty ↔ non-empty). `formKey` is how a NEW body gets in:
 * remounting is the only way to reset an uncontrolled field, so loading a saved entry or clearing
 * the canvas bumps that key rather than fighting the DOM.
 */
import type { RefObject } from "preact";
import { useRef } from "preact/hooks";

import { CHIPS, chipLine } from "./guidance";
import type { Area, CanvasBody, Idea } from "./model";
import { AREAS } from "./model";

export interface FormProps {
  body: CanvasBody;
  /** Bumped by the parent when `body` is a DIFFERENT document — remounts every field. */
  formKey: string;
  onArea: (area: Area, value: string) => void;
  onIdea: (id: string, prop: "name" | "desc" | "time" | "space", value: string) => void;
  onAddIdea: () => void;
  onRemoveIdea: (id: string) => void;
  onInfo: (key: string) => void;
  /** A saved entry is being read, not edited — every field is frozen until "Back to draft". */
  readOnly: boolean;
}

const AREA_LABEL: Record<Area, string> = {
  problem: "Problem",
  constraints: "Constraints",
  maintenance: "Maintenance",
  inputs: "Inputs",
  ret: "Return",
  errors: "Error / N/A",
  tests: "Tests",
};

/** `ideas` is an area of the canvas but not a text field, so its info key is spelled here rather
 *  than derived from `AREAS`. */
const INFO_KEY: Record<Area, string> = {
  problem: "problem",
  constraints: "constraints",
  maintenance: "maintenance",
  inputs: "inputs",
  ret: "ret",
  errors: "errors",
  tests: "tests",
};

function InfoButton({ area, onInfo }: { area: string; label: string; onInfo: (key: string) => void }) {
  return (
    <button
      class="pcanvas__info"
      type="button"
      title="What goes here"
      aria-label={`What is expected in ${area}`}
      onClick={() => onInfo(area)}
    >
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true">
        <circle cx="12" cy="12" r="10" />
        <path d="M12 16v-4 M12 8h.01" />
      </svg>
    </button>
  );
}

/** One area card: header (name · ℹ️ · optional hint), textarea, optional chip row. */
function AreaCard({
  area,
  body,
  formKey,
  onArea,
  onInfo,
  readOnly,
  hint,
  size,
  fieldRef,
}: {
  area: Area;
  body: CanvasBody;
  formKey: string;
  onArea: FormProps["onArea"];
  onInfo: FormProps["onInfo"];
  readOnly: boolean;
  hint?: string;
  size: "sm" | "md" | "lg";
  fieldRef: RefObject<HTMLTextAreaElement>;
}) {
  const chips = CHIPS[area] ?? [];
  const label = AREA_LABEL[area];
  return (
    <div class="pcanvas__card">
      <div class="pcanvas__card-head">
        <span class="pcanvas__card-title">{label}</span>
        <InfoButton area={INFO_KEY[area]} label={label} onInfo={onInfo} />
        {hint && <span class="pcanvas__card-hint">{hint}</span>}
      </div>
      <textarea
        key={formKey}
        ref={fieldRef}
        class={`pcanvas__field pcanvas__field--${size}`}
        aria-label={label}
        defaultValue={body[area]}
        readOnly={readOnly}
        onInput={(event) => onArea(area, (event.currentTarget as HTMLTextAreaElement).value)}
      />
      {chips.length > 0 && !readOnly && (
        <div class="pcanvas__chips">
          {chips.map((chip) => (
            <button
              key={chip}
              class="pcanvas__chip"
              type="button"
              onClick={() => {
                // Written through the DOM, not through a re-render: the field is uncontrolled, so
                // the element's value IS the buffer. Focusing and parking the caret at the end is
                // the point of the chip — it plants a prompt you then finish typing.
                const field = fieldRef.current;
                if (!field) return;
                const next = chipLine(field.value, chip);
                field.value = next;
                onArea(area, next);
                field.focus();
                field.selectionStart = next.length;
                field.selectionEnd = next.length;
              }}
            >
              + {chip}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

/** One idea row: name, description, and the two complexity boxes that make it a trade-off rather
 *  than a preference. */
function IdeaRow({
  idea,
  formKey,
  onIdea,
  onRemove,
  readOnly,
  removable,
}: {
  idea: Idea;
  formKey: string;
  onIdea: FormProps["onIdea"];
  onRemove: () => void;
  readOnly: boolean;
  removable: boolean;
}) {
  return (
    <div class="pcanvas__idea">
      <div class="pcanvas__idea-main">
        <div class="pcanvas__idea-head">
          <input
            key={`${formKey}-n-${idea.id}`}
            class="pcanvas__idea-name"
            aria-label="Idea name"
            defaultValue={idea.name}
              readOnly={readOnly}
            onInput={(event) => onIdea(idea.id, "name", (event.currentTarget as HTMLInputElement).value)}
          />
          {!readOnly && removable && (
            <button class="pcanvas__idea-drop" type="button" title="Remove idea" aria-label="Remove idea" onClick={onRemove}>
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true">
                <path d="M18 6L6 18 M6 6l12 12" />
              </svg>
            </button>
          )}
        </div>
        <textarea
          key={`${formKey}-d-${idea.id}`}
          class="pcanvas__field pcanvas__field--sm"
          aria-label="Idea description"
          defaultValue={idea.desc}
          readOnly={readOnly}
          onInput={(event) => onIdea(idea.id, "desc", (event.currentTarget as HTMLTextAreaElement).value)}
        />
      </div>
      <div class="pcanvas__idea-cost">
        <label class="pcanvas__cost">
          <span class="pcanvas__cost-key" title="Time complexity">T</span>
          <input
            key={`${formKey}-t-${idea.id}`}
            aria-label="Time complexity"
            defaultValue={idea.time}
            placeholder="O(n²)"
            readOnly={readOnly}
            onInput={(event) => onIdea(idea.id, "time", (event.currentTarget as HTMLInputElement).value)}
          />
        </label>
        <label class="pcanvas__cost">
          <span class="pcanvas__cost-key" title="Space complexity">S</span>
          <input
            key={`${formKey}-s-${idea.id}`}
            aria-label="Space complexity"
            defaultValue={idea.space}
            placeholder="O(1)"
            readOnly={readOnly}
            onInput={(event) => onIdea(idea.id, "space", (event.currentTarget as HTMLInputElement).value)}
          />
        </label>
      </div>
    </div>
  );
}

export function CanvasForm({
  body,
  formKey,
  onArea,
  onIdea,
  onAddIdea,
  onRemoveIdea,
  onInfo,
  readOnly,
}: FormProps) {
  // One ref per area, minted once — the chips write through them.
  const refs = useRef<Record<Area, RefObject<HTMLTextAreaElement>>>(
    Object.fromEntries(AREAS.map((area) => [area, { current: null }])) as Record<
      Area,
      RefObject<HTMLTextAreaElement>
    >,
  );
  const card = (area: Area, size: "sm" | "md" | "lg", hint?: string) => (
    <AreaCard
      area={area}
      body={body}
      formKey={formKey}
      onArea={onArea}
      onInfo={onInfo}
      readOnly={readOnly}
      hint={hint}
      size={size}
      fieldRef={refs.current[area]}
    />
  );

  return (
    <div class="pcanvas__grid">
      {card("problem", "sm")}

      <div class="pcanvas__row pcanvas__row--2">
        {card("constraints", "md", "Ask, don't assume")}
        {card("maintenance", "md")}
      </div>

      <div class="pcanvas__row pcanvas__row--3">
        {card("inputs", "sm")}
        {card("ret", "sm")}
        {card("errors", "sm")}
      </div>

      <div class="pcanvas__card">
        <div class="pcanvas__card-head">
          <span class="pcanvas__card-title">Ideas</span>
          <InfoButton area="ideas" label="Ideas" onInfo={onInfo} />
          <span class="pcanvas__card-spacer" />
          {!readOnly && (
            <button class="pcanvas__chip pcanvas__chip--add" type="button" onClick={onAddIdea}>
              + Idea
            </button>
          )}
        </div>
        {body.ideas.map((idea) => (
          <IdeaRow
            key={idea.id}
            idea={idea}
            formKey={formKey}
            onIdea={onIdea}
            onRemove={() => onRemoveIdea(idea.id)}
            readOnly={readOnly}
            removable={body.ideas.length > 1}
          />
        ))}
      </div>

      {card("tests", "md")}
    </div>
  );
}
