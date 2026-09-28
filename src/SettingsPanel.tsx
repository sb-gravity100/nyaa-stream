import { useEffect, useRef } from "preact/hooks";
import type { JSX } from "preact";
import { DEFAULT_SUBTITLE_STYLE, resetSettings, updateSettings, useSettings, type PreferredResolution, type SubtitleStyle } from "./settings";
import { CloseIcon } from "./icons";

interface Props {
  onClose: () => void;
}

// Fonts libass can resolve: the bundled default, plus common system fonts
// found through the Local Font Access API (see assRenderer.ts's
// `queryFonts: "local"`). Anything unavailable falls back to the default.
const FONT_CHOICES = ["Liberation Sans", "Arial", "Segoe UI", "Verdana", "Trebuchet MS", "Tahoma", "Georgia", "Yu Gothic"];

const LANGUAGE_CHOICES: [string, string][] = [
  ["en", "English"],
  ["es", "Spanish"],
  ["pt", "Portuguese"],
  ["fr", "French"],
  ["de", "German"],
  ["it", "Italian"],
  ["ru", "Russian"],
  ["ar", "Arabic"],
  ["id", "Indonesian"],
  ["zh", "Chinese"],
  ["ja", "Japanese"],
];

function Toggle({ checked, onChange, label, hint }: { checked: boolean; onChange: (v: boolean) => void; label: string; hint?: string }) {
  return (
    <label class="setting-row">
      <span class="setting-text">
        <span class="setting-label">{label}</span>
        {hint && <span class="setting-hint">{hint}</span>}
      </span>
      <input type="checkbox" class="switch" checked={checked} onChange={(e) => onChange((e.target as HTMLInputElement).checked)} />
    </label>
  );
}

function Row({ label, hint, children }: { label: string; hint?: string; children: JSX.Element | JSX.Element[] }) {
  return (
    <label class="setting-row">
      <span class="setting-text">
        <span class="setting-label">{label}</span>
        {hint && <span class="setting-hint">{hint}</span>}
      </span>
      <span class="setting-control">{children}</span>
    </label>
  );
}

/** CSS approximation of the libass output, for immediate feedback while
 * adjusting - the real renderer uses the same values via ASS styles. */
function SubtitlePreview({ style }: { style: SubtitleStyle }) {
  const outline = style.background ? 0 : Math.max(0.5, style.outlineWidth * 3);
  const bgAlpha = style.backgroundOpacity / 100;
  const hex = style.backgroundColor.replace("#", "");
  const bg = `rgba(${parseInt(hex.slice(0, 2), 16)}, ${parseInt(hex.slice(2, 4), 16)}, ${parseInt(hex.slice(4, 6), 16)}, ${bgAlpha})`;
  return (
    <div class="subtitle-preview" aria-hidden="true">
      <div class="subtitle-preview-frame" />
      <span
        class="subtitle-preview-text"
        style={{
          fontFamily: `"${style.fontFamily}", sans-serif`,
          fontSize: `${style.sizePercent * 0.034}em`,
          fontWeight: style.bold ? 700 : 400,
          color: style.color,
          bottom: `${style.marginPercent}%`,
          background: style.background ? bg : "transparent",
          padding: style.background ? "0.08em 0.35em" : 0,
          WebkitTextStroke: outline ? `${outline}px ${style.outlineColor}` : undefined,
          paintOrder: "stroke fill",
          textShadow: style.shadow ? `${style.shadow}px ${style.shadow}px 0 rgba(0,0,0,0.6)` : undefined,
        }}
      >
        I'll carry the sword to the end of the journey.
      </span>
    </div>
  );
}

export function SettingsPanel({ onClose }: Props) {
  const settings = useSettings();
  const style = settings.subtitleStyle;
  const panelRef = useRef<HTMLDivElement>(null);

  function setStyle(patch: Partial<SubtitleStyle>) {
    updateSettings({ subtitleStyle: { ...style, ...patch } });
  }

  useEffect(() => {
    panelRef.current?.focus();
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div class="settings-backdrop" onClick={(e) => e.target === e.currentTarget && onClose()}>
      <div class="settings-panel" role="dialog" aria-modal="true" aria-labelledby="settings-title" tabIndex={-1} ref={panelRef}>
        <header class="settings-header">
          <h2 id="settings-title">Settings</h2>
          <button class="icon-button" onClick={onClose} aria-label="Close settings">
            <CloseIcon />
          </button>
        </header>

        <div class="settings-body">
          <section class="settings-section">
            <h3>Playback</h3>
            <Toggle label="Resume where you left off" checked={settings.resumePlayback} onChange={(v) => updateSettings({ resumePlayback: v })} />
            <Toggle
              label="Play the next episode automatically"
              hint="Counts down when an episode ends"
              checked={settings.autoplayNext}
              onChange={(v) => updateSettings({ autoplayNext: v })}
            />
            <Toggle
              label="Stick with the same fansub group"
              hint="Prefers the group you last watched a show with"
              checked={settings.rememberFansubGroup}
              onChange={(v) => updateSettings({ rememberFansubGroup: v })}
            />
            <Row label="Preferred quality" hint="Used when picking a release automatically">
              <select
                value={settings.preferredResolution}
                onChange={(e) => updateSettings({ preferredResolution: (e.target as HTMLSelectElement).value as PreferredResolution })}
              >
                <option value="any">Most seeded</option>
                <option value="2160">2160p</option>
                <option value="1080">1080p</option>
                <option value="720">720p</option>
                <option value="480">480p</option>
              </select>
            </Row>
          </section>

          <section class="settings-section">
            <h3>Subtitles</h3>
            <Toggle label="Turn subtitles on by default" checked={settings.subtitlesEnabled} onChange={(v) => updateSettings({ subtitlesEnabled: v })} />
            <Row label="Language">
              <select value={settings.subtitleLanguage} onChange={(e) => updateSettings({ subtitleLanguage: (e.target as HTMLSelectElement).value })}>
                {LANGUAGE_CHOICES.map(([code, name]) => (
                  <option key={code} value={code}>
                    {name}
                  </option>
                ))}
              </select>
            </Row>

            <SubtitlePreview style={style} />

            <Row label="Font">
              <select value={style.fontFamily} onChange={(e) => setStyle({ fontFamily: (e.target as HTMLSelectElement).value })}>
                {FONT_CHOICES.map((f) => (
                  <option key={f} value={f}>
                    {f}
                  </option>
                ))}
              </select>
            </Row>
            <Row label="Size">
              <input type="range" min={3} max={10} step={0.25} value={style.sizePercent} onInput={(e) => setStyle({ sizePercent: Number((e.target as HTMLInputElement).value) })} />
            </Row>
            <Toggle label="Bold" checked={style.bold} onChange={(v) => setStyle({ bold: v })} />
            <Row label="Text color">
              <input type="color" value={style.color} onInput={(e) => setStyle({ color: (e.target as HTMLInputElement).value })} />
            </Row>
            <Toggle label="Background box instead of outline" checked={style.background} onChange={(v) => setStyle({ background: v })} />
            {style.background ? (
              <>
                <Row label="Box color">
                  <input type="color" value={style.backgroundColor} onInput={(e) => setStyle({ backgroundColor: (e.target as HTMLInputElement).value })} />
                </Row>
                <Row label="Box opacity">
                  <input
                    type="range"
                    min={0}
                    max={100}
                    step={5}
                    value={style.backgroundOpacity}
                    onInput={(e) => setStyle({ backgroundOpacity: Number((e.target as HTMLInputElement).value) })}
                  />
                </Row>
              </>
            ) : (
              <>
                <Row label="Outline color">
                  <input type="color" value={style.outlineColor} onInput={(e) => setStyle({ outlineColor: (e.target as HTMLInputElement).value })} />
                </Row>
                <Row label="Outline width">
                  <input type="range" min={0} max={1.2} step={0.05} value={style.outlineWidth} onInput={(e) => setStyle({ outlineWidth: Number((e.target as HTMLInputElement).value) })} />
                </Row>
              </>
            )}
            <Row label="Shadow">
              <input type="range" min={0} max={4} step={0.5} value={style.shadow} onInput={(e) => setStyle({ shadow: Number((e.target as HTMLInputElement).value) })} />
            </Row>
            <Row label="Distance from bottom">
              <input type="range" min={0} max={20} step={1} value={style.marginPercent} onInput={(e) => setStyle({ marginPercent: Number((e.target as HTMLInputElement).value) })} />
            </Row>
            <Toggle
              label="Also restyle styled (ASS) subtitles"
              hint="Only their dialogue lines. Signs and typesetting keep their look."
              checked={style.applyToStyled}
              onChange={(v) => setStyle({ applyToStyled: v })}
            />
            <div class="settings-inline-actions">
              <button class="link-button" onClick={() => updateSettings({ subtitleStyle: DEFAULT_SUBTITLE_STYLE })}>
                Reset subtitle style
              </button>
            </div>
          </section>

          <section class="settings-section">
            <h3>Player shortcuts</h3>
            <dl class="shortcut-list">
              <div><dt>Space / K</dt><dd>Play or pause</dd></div>
              <div><dt>← → / J L</dt><dd>Seek 5s / 10s</dd></div>
              <div><dt>↑ ↓</dt><dd>Volume</dd></div>
              <div><dt>C</dt><dd>Cycle subtitles</dd></div>
              <div><dt>Z / X</dt><dd>Subtitle delay −/+ 0.1s</dd></div>
              <div><dt>N</dt><dd>Next episode</dd></div>
              <div><dt>F / M</dt><dd>Fullscreen / mute</dd></div>
            </dl>
          </section>
        </div>

        <footer class="settings-footer">
          <button class="button button-quiet" onClick={() => resetSettings()}>
            Reset all settings
          </button>
          <button class="button button-primary" onClick={onClose}>
            Done
          </button>
        </footer>
      </div>
    </div>
  );
}
