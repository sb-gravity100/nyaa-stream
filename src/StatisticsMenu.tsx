import { useState } from "preact/hooks";

// Ported from stremio-web's Player/StatisticsMenu: peers/speed/completed at
// a glance, plus the torrent's info hash with a copy button - toggled from
// the control bar rather than always shown, same as upstream.
interface Props {
  peers: number;
  speedMbps: number;
  completedPercent: number;
  infoHash: string | null;
}

export function StatisticsMenu({ peers, speedMbps, completedPercent, infoHash }: Props) {
  const [copied, setCopied] = useState(false);

  async function copyHash() {
    if (!infoHash) return;
    try {
      await navigator.clipboard.writeText(infoHash);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // Clipboard unavailable - nothing more to do.
    }
  }

  return (
    <div class="statistics-menu">
      <div class="statistics-menu-title">Statistics</div>
      <div class="statistics-menu-stats">
        <div class="statistics-menu-stat">
          <div class="statistics-menu-label">Peers</div>
          <div class="statistics-menu-value">{peers}</div>
        </div>
        <div class="statistics-menu-stat">
          <div class="statistics-menu-label">Speed</div>
          <div class="statistics-menu-value">{speedMbps.toFixed(2)} MB/s</div>
        </div>
        <div class="statistics-menu-stat">
          <div class="statistics-menu-label">Completed</div>
          <div class="statistics-menu-value">{Math.min(completedPercent, 100).toFixed(0)}%</div>
        </div>
      </div>
      {infoHash && (
        <button class="statistics-menu-hash" onClick={copyHash} title="Copy info hash">
          <span class="statistics-menu-hash-value">{infoHash}</span>
          <span class="statistics-menu-hash-label">{copied ? "Copied" : "Copy"}</span>
        </button>
      )}
    </div>
  );
}
