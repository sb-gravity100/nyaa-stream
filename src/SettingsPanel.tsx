import { useEffect, useRef, useState } from "preact/hooks";
import { invoke } from "@tauri-apps/api/core";
import { open as pickFolder, open as pickFile } from "@tauri-apps/plugin-dialog";
import type { JSX } from "preact";
import { DEFAULT_SUBTITLE_STYLE, resetSettings, updateSettings, useSettings, type PreferredResolution, type SubtitleStyle } from "./settings";
import { CloseIcon } from "./icons";
import { exportBackup, importBackup } from "./backup";
import { checkForUpdate, installUpdate } from "./updater";
import { HelpSection } from "./HelpSection";
import type { Update } from "@tauri-apps/plugin-updater";

interface Props {
  onClose: () => void;
}

// Fonts libass can resolve: the bundled default, plus common system fonts
// found through the Local Font Access API (see assRenderer.ts's
// `queryFonts: "local"`). Anything unavailable falls back to the default.
const FONT_CHOICES = ["Gandhi Sans", "Liberation Sans", "Arial", "Segoe UI", "Verdana", "Trebuchet MS", "Tahoma", "Georgia", "Yu Gothic"];

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

interface CacheSizes {
  nyaa: number;
  thumbnails: number;
  hls: number;
  torrents: number;
}

const CACHES: { kind: keyof CacheSizes; label: string; hint: string }[] = [
  { kind: "nyaa", label: "Search cache", hint: "nyaa.si search results and torrent details" },
  { kind: "thumbnails", label: "Thumbnails", hint: "Episode thumbnails and Kitsu metadata" },
  { kind: "hls", label: "Transcoded video (HLS)", hint: "Temporary segments - also cleared on every start and exit" },
  { kind: "torrents", label: "Downloaded torrent data", hint: "Played torrents kept so re-watching and seeking back don't download again" },
];

const GB = 1024 ** 3;
/** Settings -> "Download cache" choices (bytes); 0 deletes on stop. */
const DOWNLOAD_CACHE_LIMITS = [0, 2 * GB, 5 * GB, 10 * GB, 20 * GB, 50 * GB];

interface DownloadCacheStatus {
  usedBytes: number;
  limitBytes: number;
  entries: number;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  if (bytes < 1024 ** 3) return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
  return `${(bytes / 1024 ** 3).toFixed(2)} GB`;
}

/** Sizes of the on-disk caches with a clear button each. */
function StorageSection() {
  const [sizes, setSizes] = useState<CacheSizes | null>(null);
  const [downloads, setDownloads] = useState<DownloadCacheStatus | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  async function refresh() {
    try {
      setSizes(await invoke<CacheSizes>("get_cache_sizes"));
      setDownloads(await invoke<DownloadCacheStatus>("download_cache_status"));
    } catch (err) {
      console.warn("[settings] cache sizes unavailable", { err: String(err) });
    }
  }

  async function setLimit(limitBytes: number) {
    console.info("[settings] download cache limit", { limitBytes });
    setBusy("limit");
    setMessage(null);
    try {
      await invoke("set_download_cache_limit", { limitBytes });
    } catch (err) {
      setMessage(String(err));
    }
    await refresh();
    setBusy(null);
  }

  useEffect(() => {
    void refresh();
  }, []);

  async function clear(kind: keyof CacheSizes) {
    setBusy(kind);
    setMessage(null);
    try {
      await invoke("clear_cache", { kind });
    } catch (err) {
      setMessage(String(err));
    }
    await refresh();
    setBusy(null);
  }

  if (!sizes) return null;
  return (
    <section class="settings-section">
      <h3>Storage</h3>
      {downloads && (
        <div class="setting-row">
          <span class="setting-text">
            <span class="setting-label">Download cache</span>
            <span class="setting-hint">
              {`${formatBytes(downloads.usedBytes)} in ${downloads.entries} torrent${downloads.entries === 1 ? "" : "s"}. Oldest are deleted past the limit; Continue watching episodes are kept.`}
            </span>
          </span>
          <span class="setting-control">
            <select
              value={String(downloads.limitBytes)}
              disabled={busy != null}
              onChange={(e) => void setLimit(Number((e.target as HTMLSelectElement).value))}
            >
              {[...new Set([...DOWNLOAD_CACHE_LIMITS, downloads.limitBytes])]
                .sort((a, b) => a - b)
                .map((bytes) => (
                  <option key={bytes} value={String(bytes)}>
                    {bytes === 0 ? "Delete on stop" : `${+(bytes / GB).toFixed(1)} GB`}
                  </option>
                ))}
            </select>
          </span>
        </div>
      )}
      {CACHES.map(({ kind, label, hint }) => (
        <div class="setting-row" key={kind}>
          <span class="setting-text">
            <span class="setting-label">{label}</span>
            <span class="setting-hint">{hint}</span>
          </span>
          <span class="setting-control">
            <span class="setting-value">{formatBytes(sizes[kind])}</span>
            <button class="button button-quiet" disabled={busy != null || sizes[kind] === 0} onClick={() => void clear(kind)}>
              {busy === kind ? "Clearing…" : "Clear"}
            </button>
          </span>
        </div>
      ))}
      {message && <div class="setting-hint">{message}</div>}
    </section>
  );
}

/** Checks GitHub Releases for a newer signed build and installs it. */
function UpdateSection() {
  const [update, setUpdate] = useState<Update | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function run(action: () => Promise<string | null>) {
    setBusy(true);
    setMessage(null);
    try {
      setMessage(await action());
    } catch (err) {
      console.warn("[updater] failed", { err: String(err) });
      setMessage(err instanceof Error ? err.message : String(err));
    }
    setBusy(false);
  }

  return (
    <section class="settings-section">
      <h3>Updates</h3>
      <div class="setting-row">
        <span class="setting-text">
          <span class="setting-label">nyaa-stream v{__APP_VERSION__}</span>
          <span class="setting-hint">{message ?? (update ? `v${update.version} is available` : "Checks GitHub Releases")}</span>
        </span>
        <span class="setting-control">
          {update ? (
            <button
              class="button button-primary"
              disabled={busy}
              onClick={() => void run(async () => { await installUpdate(update, (p) => setMessage(p === null ? "Downloading…" : `Downloading ${p}%`)); return null; })}
            >
              Install and restart
            </button>
          ) : (
            <button
              class="button button-quiet"
              disabled={busy}
              onClick={() =>
                void run(async () => {
                  const found = await checkForUpdate();
                  setUpdate(found);
                  return found ? null : "You're up to date";
                })
              }
            >
              Check for updates
            </button>
          )}
        </span>
      </div>
    </section>
  );
}

/** Library, progress and settings <-> a file in Documents, since they live
 * in webview storage that a data clear would take with it. */
function BackupSection() {
  const [message, setMessage] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function run(action: () => Promise<string>) {
    setBusy(true);
    setMessage(null);
    try {
      setMessage(await action());
    } catch (err) {
      setMessage(err instanceof Error ? err.message : String(err));
    }
    setBusy(false);
  }

  return (
    <section class="settings-section">
      <h3>Backup</h3>
      <div class="setting-row">
        <span class="setting-text">
          <span class="setting-label">Library, progress and settings</span>
          <span class="setting-hint">Saved to nyaa-stream-backup.json in your Documents folder</span>
        </span>
        <span class="setting-control">
          <button class="button button-quiet" disabled={busy} onClick={() => void run(async () => `Saved to ${await exportBackup()}`)}>
            Export
          </button>
          <button
            class="button button-quiet"
            disabled={busy}
            onClick={() =>
              void run(async () => {
                const count = await importBackup();
                window.setTimeout(() => window.location.reload(), 800);
                return `Restored ${count} entries - reloading…`;
              })
            }
          >
            Import
          </button>
        </span>
      </div>
      <div class="setting-row">
        <span class="setting-text">
          <span class="setting-label">nyaa.si release database</span>
          <span class="setting-hint">Merge a SQLite release database (nyaa-stream's format) into the local one - searches find its releases instantly and offline</span>
        </span>
        <span class="setting-control">
          <button
            class="button button-quiet"
            disabled={busy}
            onClick={() =>
              void run(async () => {
                const path = await pickFile({ multiple: false, directory: false, filters: [{ name: "SQLite database", extensions: ["db", "sqlite", "sqlite3"] }] });
                if (typeof path !== "string") return "Import cancelled";
                console.info("[settings] importing nyaa release database", { path });
                const stats = await invoke<{ total: number; added: number; updated: number }>("import_nyaa_database", { path });
                console.info("[settings] nyaa release database imported", stats);
                return `Imported ${stats.total} releases: ${stats.added} new, ${stats.updated} updated`;
              })
            }
          >
            Import
          </button>
        </span>
      </div>
      {message && <div class="setting-hint">{message}</div>}
    </section>
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
  // Percent-of-video-height values -> container-width units (the frame is
  // 16:9, so 1% of height = 0.5625cqw). Text stroke paints half inside the
  // glyph, hence doubled to match libass's outside-only outline.
  const toCqw = (percentOfHeight: number) => `${percentOfHeight * 0.5625}cqw`;
  const outline = style.background ? 0 : style.outlineWidth * 2;
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
          fontSize: toCqw(style.sizePercent),
          fontWeight: style.bold ? 700 : 400,
          color: style.color,
          bottom: `${style.marginPercent}%`,
          background: style.background ? bg : "transparent",
          padding: style.background ? "0.08em 0.35em" : 0,
          WebkitTextStroke: outline ? `${toCqw(outline)} ${style.outlineColor}` : undefined,
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
  const [popularFansubbers, setPopularFansubbers] = useState<string[]>([]);
  const [defaultScreenshotFolder, setDefaultScreenshotFolder] = useState("");

  useEffect(() => {
    // Suggestions only exist in the desktop app (the backend counts group tags from real searches).
    invoke<string>("default_screenshot_folder")
      .then(setDefaultScreenshotFolder)
      .catch(() => undefined);
    invoke<string[]>("get_popular_fansubbers")
      .then(setPopularFansubbers)
      .catch(() => undefined);
  }, []);

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
              label="Hardware video decoding"
              hint="Turn off if video turns blocky or smeared until you seek"
              checked={settings.hardwareDecoding}
              onChange={(v) => updateSettings({ hardwareDecoding: v })}
            />
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
            <Toggle
              label="Hide unlisted sources"
              hint="Only show episodes AniList/Kitsu know about - hides batches, unrecognized titles and extra episode numbers"
              checked={settings.hideUnlistedSources}
              onChange={(v) => updateSettings({ hideUnlistedSources: v })}
            />
            <Row label="libmpv path" hint="Leave empty to use the bundled libmpv-2.dll. Point at a libmpv-2.dll (or its folder) to use another build. Applies the next time a player opens.">
              <input
                type="text"
                class="setting-text-input"
                placeholder="libmpv-2.dll (or its folder)"
                value={settings.mpvPath}
                onChange={(e) => updateSettings({ mpvPath: (e.target as HTMLInputElement).value.trim() })}
              />
            </Row>
            <Row label="Screenshot folder" hint="Where X saves frames. Leave empty for Pictures
yaa-stream screenshots.">
              <input
                type="text"
                class="setting-text-input"
                placeholder={defaultScreenshotFolder || "Pictures\nyaa-stream screenshots"}
                value={settings.screenshotFolder}
                onChange={(e) => updateSettings({ screenshotFolder: (e.target as HTMLInputElement).value.trim() })}
              />
              <button
                class="button button-quiet"
                type="button"
                onClick={async () => {
                  const picked = await pickFolder({ directory: true, multiple: false, title: "Save screenshots to" }).catch(() => null);
                  if (typeof picked === "string") updateSettings({ screenshotFolder: picked });
                }}
              >
                Browse…
              </button>
            </Row>
            <Row label="Preferred fansubber" hint="Release tags to prefer when picking a source, e.g. ToonsHub CR. A release matches when its title has every word. Leave empty for no preference.">
              <input
                type="text"
                class="setting-text-input"
                placeholder="e.g. ToonsHub CR"
                list="popular-fansubbers"
                value={settings.preferredFansubber}
                onChange={(e) => updateSettings({ preferredFansubber: (e.target as HTMLInputElement).value.trim() })}
              />
            </Row>
            <datalist id="popular-fansubbers">
              {popularFansubbers.map((name) => (
                <option key={name} value={name} />
              ))}
            </datalist>
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

          <UpdateSection />

          <BackupSection />

          <StorageSection />
          <HelpSection />

          <section class="settings-section">
            <h3>Player shortcuts</h3>
            <dl class="shortcut-list">
              <div><dt>Space / K</dt><dd>Play or pause</dd></div>
              <div><dt>← → / J L</dt><dd>Seek 5s / 10s</dd></div>
              <div><dt>↑ ↓</dt><dd>Volume</dd></div>
              <div><dt>C</dt><dd>Subtitles on / off</dd></div>
              <div><dt>Hold C + scroll</dt><dd>Step through subtitle tracks</dd></div>
              <div><dt>, / .</dt><dd>Step one frame back / forward</dd></div>
              <div><dt>- / =</dt><dd>Subtitle delay −/+ 0.1s</dd></div>
              <div><dt>X</dt><dd>Save the current frame as a PNG</dd></div>
              <div><dt>N</dt><dd>Next episode</dd></div>
              <div><dt>Shift</dt><dd>Skip the opening (90s if the file marks none)</dd></div>
              <div><dt>{"[ / ] / \\"}</dt><dd>Playback speed slower / faster / reset</dd></div>
              <div><dt>A</dt><dd>A-B loop: set A, set B, clear</dd></div>
              <div><dt>E</dt><dd>Export the looped section as MP4</dd></div>
              <div><dt>M</dt><dd>Mute</dd></div>
              <div><dt>F</dt><dd>Fullscreen (anywhere in the app)</dd></div>
              <div><dt>P</dt><dd>Picture-in-picture (mini always-on-top window)</dd></div>
              <div><dt>Ctrl + C</dt><dd>Copy the current frame</dd></div>
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
