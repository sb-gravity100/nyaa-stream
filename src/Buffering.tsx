// Ported from stremio-web's Player/Buffering: a mark that fills left-to-right
// via clip-path as `progress` climbs, pulsing while incomplete, with a dim
// full-opacity copy behind it so the unfilled outline stays visible. Uses an
// original mark (a play triangle in a ring) rather than Stremio's own logo.
import { useRef } from "preact/hooks";

interface Props {
  progress: number;
}

function Mark() {
  return (
    <svg viewBox="0 0 100 100" aria-hidden="true">
      <circle cx="50" cy="50" r="44" fill="none" stroke="currentColor" stroke-width="6" />
      <path d="M40 30 L74 50 L40 70 Z" fill="currentColor" />
    </svg>
  );
}

export function Buffering({ progress }: Props) {
  // Readiness can dip (a peer drops); the fill only ever grows while this
  // indicator is up, so it never visibly runs backwards.
  const shown = useRef(0);
  shown.current = Math.max(shown.current, progress);
  return (
    <div class="player-buffering">
      <div class="player-buffering-mark player-buffering-mark-background">
        <Mark />
      </div>
      <div class="player-buffering-mark" style={{ clipPath: `inset(0 ${100 - shown.current}% 0 0)` }}>
        <Mark />
      </div>
    </div>
  );
}
