/**
 * The line under the editor: encoding, indent, caret, length — and the four file actions.
 *
 * The caret moves on every keystroke, so it is the one piece of the page that changes that often.
 * It therefore lives in THIS component's state, fed through a sink the page hands Monaco's
 * `onCursor`, and typing re-renders a status line rather than the whole lab.
 */
import type { RefObject } from "preact";
import { useEffect, useState } from "preact/hooks";

import { DOWNLOAD, FOLDER, SAVE, SHARE } from "./icons";

export type CursorSink = (line: number, column: number, lines: number) => void;

interface Props {
  sink: RefObject<CursorSink | null>;
  tabSize: number;
  onDownload: () => void;
  onOpen: () => void;
  onSave: () => void;
  onShare: () => void;
}

export function StatusBar({ sink, tabSize, onDownload, onOpen, onSave, onShare }: Props) {
  const [at, setAt] = useState({ line: 1, column: 1, lines: 1 });

  useEffect(() => {
    sink.current = (line, column, lines) => setAt({ line, column, lines });
    return () => {
      sink.current = null;
    };
  }, [sink]);

  const action = (label: string, icon: preact.JSX.Element, onClick: () => void) => (
    <button type="button" class="clab-status__btn" aria-label={label} title={label} onClick={onClick}>
      {icon}
    </button>
  );

  return (
    <div class="clab-status">
      <span class="clab-status__item">UTF-8</span>
      <span class="clab-status__item">Spaces: {tabSize}</span>
      <span class="clab-status__item" data-codelab-caret>
        Ln {at.line}, Col {at.column}
      </span>
      <span class="clab-status__item">
        {at.lines} {at.lines === 1 ? "line" : "lines"}
      </span>
      <span class="clab-status__sp"></span>
      {action("Download this file", DOWNLOAD, onDownload)}
      {action("Open a file in a new tab", FOLDER, onOpen)}
      {action("Save in this browser (⌘S)", SAVE, onSave)}
      {action("Copy a share link", SHARE, onShare)}
    </div>
  );
}
