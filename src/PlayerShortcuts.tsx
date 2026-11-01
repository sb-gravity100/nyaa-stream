import { useEffect, useRef } from "preact/hooks";
import type { JSX } from "preact";

const SHORTCUTS = [
  ["Space / K", "Play or pause"],
  ["← → / J L", "Seek 5s / 10s"],
  ["↑ ↓", "Volume"],
  ["C", "Subtitles on / off"],
  ["Hold C + scroll", "Step through subtitle tracks"],
  [", / .", "Step one frame back / forward"],
  ["- / =", "Subtitle delay −/+ 0.1s"],
  ["X", "Save the current frame as a PNG"],
  ["N", "Next episode"],
  ["Shift", "Skip the opening (90s if the file marks none)"],
  ["[ / ] / \\", "Playback speed slower / faster / reset"],
  ["A", "A-B loop: set A, set B, clear"],
  ["E", "Export the looped section as MP4"],
  ["M", "Mute"],
  ["F", "Fullscreen (anywhere in the app)"],
  ["P", "Picture-in-picture (mini always-on-top window)"],
  ["Ctrl + C", "Copy the current frame"],
] as const;

export function PlayerShortcutsList() {
  return (
    <dl class="shortcut-list">
      {SHORTCUTS.map(([key, action]) => (
        <div key={key}><dt>{key}</dt><dd>{action}</dd></div>
      ))}
    </dl>
  );
}

export function PlayerShortcutsDialog({ onClose }: { onClose: () => void }) {
  const dialogRef = useRef<HTMLElement>(null);
  const closeRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    closeRef.current?.focus();
    console.debug("[player] keyboard shortcuts dialog opened");
  }, []);

  function handleKeyDown(event: JSX.TargetedKeyboardEvent<HTMLElement>) {
    if (event.key === "Escape") {
      event.stopPropagation();
      onClose();
      return;
    }
    if (event.key !== "Tab") return;
    const dialog = dialogRef.current;
    if (!dialog) return;
    const focusable = Array.from(dialog.querySelectorAll<HTMLElement>(
      "button:not([disabled]), a[href], input:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex='-1'])",
    ));
    if (focusable.length === 0) {
      event.preventDefault();
      return;
    }
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }

  return (
    <div class="player-shortcuts-backdrop" onClick={(event) => event.target === event.currentTarget && onClose()}>
      <section
        ref={dialogRef}
        class="player-shortcuts-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="player-shortcuts-title"
        tabIndex={-1}
        onKeyDown={handleKeyDown}
      >
        <header class="player-shortcuts-header">
          <h2 id="player-shortcuts-title">Player shortcuts</h2>
          <button ref={closeRef} class="icon-button" type="button" onClick={onClose} aria-label="Close player shortcuts">
            <span aria-hidden="true">×</span>
          </button>
        </header>
        <div class="player-shortcuts-body"><PlayerShortcutsList /></div>
      </section>
    </div>
  );
}
