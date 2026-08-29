// Ported from stremio-web's Player/Buffering: a mark that fills left-to-right
// via clip-path as `progress` climbs, pulsing while incomplete, with a dim
// full-opacity copy behind it so the unfilled outline stays visible. Uses an
// original mark (a play triangle in a ring) rather than Stremio's own logo.
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
  return (
    <div class="player-buffering">
      <div class="player-buffering-mark player-buffering-mark-background">
        <Mark />
      </div>
      <div class="player-buffering-mark" style={{ clipPath: `inset(0 ${100 - progress}% 0 0)` }}>
        <Mark />
      </div>
    </div>
  );
}
