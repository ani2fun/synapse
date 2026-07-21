/**
 * The quiz card (port of client/src/quiz/mod.rs — oracle: `quiz/QuizCard` + `QuizBlocks`, step
 * 16, a thin flat feature): one check-your-understanding question rendered from a ```quiz fence.
 * Select an option, Check — the right answer tints green wherever it is, a wrong pick tints red,
 * and the verdict line says which; Try again resets. All state is two hooks; nothing leaves the
 * component (quizzes are ungraded prose furniture, not submissions).
 */
import { render, h } from "preact";
import { useState } from "preact/hooks";

interface Quiz {
  prompt: string;
  options: string[];
  answer: string;
  input: string | null;
}

function decodedAttr(element: Element, name: string): string | null {
  const raw = element.getAttribute(name);
  if (raw == null) return null;
  try {
    return decodeURIComponent(raw);
  } catch {
    return null;
  }
}

/** Decode `data-quiz` (render.ts already shape-checked it at parse time; this mirrors that
 *  check rather than trusting it blindly — a malformed card is skipped, never a crash). */
function parseQuizAttr(element: Element): Quiz | null {
  const raw = decodedAttr(element, "data-quiz");
  if (raw == null) return null;
  try {
    const data = JSON.parse(raw) as Partial<Quiz>;
    if (
      typeof data.prompt !== "string" ||
      !Array.isArray(data.options) ||
      data.options.some((o) => typeof o !== "string") ||
      typeof data.answer !== "string"
    ) {
      return null;
    }
    return {
      prompt: data.prompt,
      options: data.options,
      answer: data.answer,
      input: typeof data.input === "string" ? data.input : null,
    };
  } catch {
    return null;
  }
}

export function hydrateQuizzes(root: ParentNode): number {
  let count = 0;
  for (const element of root.querySelectorAll("div.quiz-block")) {
    const quiz = parseQuizAttr(element);
    if (!quiz) continue;
    const host = element as HTMLElement;
    host.replaceChildren();
    render(h(QuizCard, { quiz }), host);
    count += 1;
  }
  return count;
}

function QuizCard({ quiz }: { quiz: Quiz }) {
  const [selected, setSelected] = useState<number | null>(null);
  const [checked, setChecked] = useState(false);

  return (
    <div class="quiz not-prose">
      <div class="quiz__head">
        <span class="wb__eyebrow">
          <span class="wb__prompt">?</span> Quiz
        </span>
        <p class="quiz__prompt">{quiz.prompt}</p>
      </div>
      {quiz.input != null && (
        <pre class="quiz__input">
          <code>{quiz.input}</code>
        </pre>
      )}
      <div class="quiz__options">
        {quiz.options.map((option, i) => {
          const isAnswer = option === quiz.answer;
          const classes = [
            "quiz__option",
            selected === i && !checked ? "quiz__option--selected" : "",
            checked && isAnswer ? "quiz__option--right" : "",
            checked && selected === i && !isAnswer ? "quiz__option--wrong" : "",
          ]
            .filter(Boolean)
            .join(" ");
          return (
            <button type="button" class={classes} disabled={checked} onClick={() => setSelected(i)}>
              {option}
            </button>
          );
        })}
      </div>
      <div class="quiz__foot">
        {checked ? (
          <div class="quiz__verdict">
            {selected != null && quiz.options[selected] === quiz.answer ? (
              <span class="quiz__verdict-ok">Correct ✓</span>
            ) : (
              <span class="quiz__verdict-no">{`Not quite — the answer is “${quiz.answer}”`}</span>
            )}
            <button
              type="button"
              class="quiz__again"
              onClick={() => {
                setChecked(false);
                setSelected(null);
              }}
            >
              Try again
            </button>
          </div>
        ) : (
          <button type="button" class="quiz__check" disabled={selected == null} onClick={() => setChecked(true)}>
            Check
          </button>
        )}
      </div>
    </div>
  );
}
