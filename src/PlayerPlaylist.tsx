import { useEffect, useRef } from "preact/hooks";
import { memo } from "preact/compat";
import type { Ref } from "preact";
import type { ProgressEntry } from "./watchProgress";
import { CheckIcon, CloseIcon, PlayIcon } from "./icons";

export interface PlaylistItem {
  /** MediaPage group key ("Episode 5", "Batch", ...). */
  key: string;
  episode: number | null;
  thumbnail: string | null;
  releaseCount: number;
  progress: ProgressEntry | null;
}

interface Props {
  title: string;
  items: PlaylistItem[];
  currentKey: string;
  open: boolean;
  onSelect: (key: string) => void;
  onClose: () => void;
}

/**
 * Slide-in episode list on the right edge of the player - switches
 * episodes without leaving playback. Mirrors the MediaPage list: same
 * thumbnails, progress bars and watched state.
 *
 * Stays mounted while closed (opening only toggles a class), and both the
 * panel and each row are memoized: PlayerView re-renders several times a
 * second during playback, and none of that should touch the list.
 */
export const PlayerPlaylist = memo(function PlayerPlaylist({ title, items, currentKey, open, onSelect, onClose }: Props) {
  const currentRef = useRef<HTMLButtonElement>(null);

  // Bring the playing episode into view each time the panel opens.
  useEffect(() => {
    if (open) currentRef.current?.scrollIntoView({ block: "center" });
  }, [open]);

  return (
    <aside class={`player-playlist${open ? " open" : ""}`} aria-label="Episodes" aria-hidden={!open}>
      <header class="player-playlist-header">
        <div>
          <div class="player-playlist-title">Episodes</div>
          <div class="player-playlist-sub">{title}</div>
        </div>
        <button class="player-icon-button" onClick={onClose} aria-label="Close episode list" tabIndex={open ? 0 : -1}>
          <CloseIcon size={18} />
        </button>
      </header>
      <div class="player-playlist-items">
        {items.map((item) => {
          const current = item.key === currentKey;
          return (
            <PlaylistRow
              key={item.key}
              item={item}
              current={current}
              focusable={open}
              onSelect={onSelect}
              buttonRef={current ? currentRef : undefined}
            />
          );
        })}
      </div>
    </aside>
  );
});

interface RowProps {
  item: PlaylistItem;
  current: boolean;
  focusable: boolean;
  onSelect: (key: string) => void;
  buttonRef?: Ref<HTMLButtonElement>;
}

const PlaylistRow = memo(function PlaylistRow({ item, current, focusable, onSelect, buttonRef }: RowProps) {
  const watched = item.progress?.completed ?? false;
  const fraction = item.progress && !watched ? Math.min(1, item.progress.position / item.progress.duration) : 0;
  return (
    <button
      ref={buttonRef}
      class={`player-playlist-item${current ? " current" : ""}${watched ? " watched" : ""}`}
      onClick={() => !current && onSelect(item.key)}
      aria-current={current ? "true" : undefined}
      tabIndex={focusable ? 0 : -1}
    >
      <span class="player-playlist-thumb">
        {item.thumbnail ? <img src={item.thumbnail} alt="" loading="lazy" decoding="async" /> : <span class="player-playlist-number">{item.episode ?? "All"}</span>}
        {current ? (
          <span class="player-playlist-now">
            <span class="eq" aria-hidden="true">
              <i />
              <i />
              <i />
            </span>
          </span>
        ) : (
          <span class="player-playlist-play">
            <PlayIcon size={18} />
          </span>
        )}
        {fraction > 0 && (
          <span class="video-progress">
            <span style={{ width: `${fraction * 100}%` }} />
          </span>
        )}
      </span>
      <span class="player-playlist-text">
        <span class="player-playlist-label">{item.key}</span>
        <span class="player-playlist-meta">
          {current ? "Now playing" : watched ? "Watched" : `${item.releaseCount} ${item.releaseCount === 1 ? "release" : "releases"}`}
        </span>
      </span>
      {watched && !current && (
        <span class="player-playlist-check" aria-label="Watched">
          <CheckIcon size={14} />
        </span>
      )}
    </button>
  );
});
