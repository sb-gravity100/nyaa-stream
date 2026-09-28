// Inline SVG icons (24x24 grid, currentColor) - replaces the emoji glyphs
// the player used to render, which looked different on every platform
// font and couldn't be sized/colored consistently.
import type { JSX } from "preact";

type IconProps = JSX.SVGAttributes<SVGSVGElement> & { size?: number };

function Svg({ size = 20, children, ...rest }: IconProps & { children: JSX.Element | JSX.Element[] }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" {...rest}>
      {children}
    </svg>
  );
}

export const PlayIcon = (p: IconProps) => (
  <Svg {...p}>
    <path d="M7 4.5v15l12.5-7.5z" fill="currentColor" />
  </Svg>
);
export const PauseIcon = (p: IconProps) => (
  <Svg {...p}>
    <rect x="6" y="4.5" width="4" height="15" rx="1" fill="currentColor" stroke="none" />
    <rect x="14" y="4.5" width="4" height="15" rx="1" fill="currentColor" stroke="none" />
  </Svg>
);
export const VolumeIcon = (p: IconProps) => (
  <Svg {...p}>
    <path d="M4 9.5h3.5L12 5.5v13l-4.5-4H4z" fill="currentColor" />
    <path d="M15.5 8.5a5 5 0 0 1 0 7M18 6a8.5 8.5 0 0 1 0 12" />
  </Svg>
);
export const MuteIcon = (p: IconProps) => (
  <Svg {...p}>
    <path d="M4 9.5h3.5L12 5.5v13l-4.5-4H4z" fill="currentColor" />
    <path d="M16 9.5l5 5M21 9.5l-5 5" />
  </Svg>
);
export const FullscreenIcon = (p: IconProps) => (
  <Svg {...p}>
    <path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5" />
  </Svg>
);
export const CloseIcon = (p: IconProps) => (
  <Svg {...p}>
    <path d="M6 6l12 12M18 6L6 18" />
  </Svg>
);
export const BackIcon = (p: IconProps) => (
  <Svg {...p}>
    <path d="M15 5l-7 7 7 7" />
  </Svg>
);
export const NextIcon = (p: IconProps) => (
  <Svg {...p}>
    <path d="M5 5v14l10-7z" fill="currentColor" />
    <path d="M19 5v14" />
  </Svg>
);
export const SubtitlesIcon = (p: IconProps) => (
  <Svg {...p}>
    <rect x="3" y="5" width="18" height="14" rx="2.5" />
    <path d="M7 15h4M13 15h4M7 11.5h2M11 11.5h6" />
  </Svg>
);
export const SourcesIcon = (p: IconProps) => (
  <Svg {...p}>
    <path d="M4 6h16M4 12h16M4 18h10" />
  </Svg>
);
export const StatsIcon = (p: IconProps) => (
  <Svg {...p}>
    <path d="M5 19V11M12 19V5M19 19v-6" />
  </Svg>
);
export const SettingsIcon = (p: IconProps) => (
  <Svg {...p}>
    <circle cx="12" cy="12" r="3" />
    <path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z" />
  </Svg>
);
export const CheckIcon = (p: IconProps) => (
  <Svg {...p}>
    <path d="M5 12.5l4.5 4.5L19 7.5" />
  </Svg>
);
export const PlusIcon = (p: IconProps) => (
  <Svg {...p}>
    <path d="M12 5v14M5 12h14" />
  </Svg>
);
export const SearchIcon = (p: IconProps) => (
  <Svg {...p}>
    <circle cx="11" cy="11" r="6.5" />
    <path d="M20 20l-4.2-4.2" />
  </Svg>
);
export const EpisodesIcon = (p: IconProps) => (
  <Svg {...p}>
    <rect x="3" y="5" width="11" height="14" rx="2" />
    <path d="M17 7h4M17 12h4M17 17h4" />
  </Svg>
);
export const SkipBackIcon = (p: IconProps) => (
  <Svg {...p}>
    <path d="M4 12a8 8 0 1 0 2.4-5.7" />
    <path d="M4 4v4h4" />
    <text x="12" y="15.5" text-anchor="middle" font-size="7.5" font-weight="700" fill="currentColor" stroke="none">10</text>
  </Svg>
);
export const SkipForwardIcon = (p: IconProps) => (
  <Svg {...p}>
    <path d="M20 12a8 8 0 1 1-2.4-5.7" />
    <path d="M20 4v4h-4" />
    <text x="12" y="15.5" text-anchor="middle" font-size="7.5" font-weight="700" fill="currentColor" stroke="none">10</text>
  </Svg>
);
export const ExitFullscreenIcon = (p: IconProps) => (
  <Svg {...p}>
    <path d="M9 4v5H4M15 4v5h5M9 20v-5H4M15 20v-5h5" />
  </Svg>
);

/** App logo: a cat-eared tile with a play triangle - "nyaa" + "stream". */
export const BrandMark = () => (
  <svg class="app-brand-mark" viewBox="0 0 32 32" aria-hidden="true">
    <defs>
      <linearGradient id="brand-fill" x1="0" y1="0" x2="1" y2="1">
        <stop offset="0" stop-color="#ffa9c8" />
        <stop offset="1" stop-color="#d9578a" />
      </linearGradient>
    </defs>
    <path d="M5 11 L7.5 2.5 L13 8 H19 L24.5 2.5 L27 11 V24 A5 5 0 0 1 22 29 H10 A5 5 0 0 1 5 24 Z" fill="url(#brand-fill)" />
    <path d="M13.5 13.5 v9 l7.5 -4.5 z" fill="#2a0f1c" stroke="#2a0f1c" stroke-width="1.5" stroke-linejoin="round" />
  </svg>
);
