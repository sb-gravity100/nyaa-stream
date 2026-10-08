// Release notes shown by the "What's new" dialog after an in-app update
// (see WhatsNew.tsx). Newest first, written for viewers, not developers.

export interface ChangelogEntry {
  version: string;
  date: string;
  features: string[];
  fixes: string[];
}

export const CHANGELOG: ChangelogEntry[] = [
  {
    version: "0.4.2",
    date: "2026-10-08",
    features: [
      "Much better at reading release names: seasons written as II/III/IV, season packs like \"S01-S04\" or \"Season 1-3\", \"01 ~ 24\" batches, Chinese and Japanese episode numbers, and 4-digit episodes.",
      "Movies and OVAs/specials get their own groups instead of \"Unknown\".",
      "Settings → Backup → nyaa.si release database: import a release database to search those releases instantly, even offline.",
    ],
    fixes: [
      "Audio channels, frame rates and \"H 264\" in release names are no longer mistaken for episode numbers.",
      "An S01E06-style number now wins over a different season mentioned elsewhere in the name.",
      "This What's new window now appears after updating to 0.4.x, and after updates installed outside the app.",
    ],
  },
  {
    version: "0.4.1",
    date: "2026-10-04",
    features: [],
    fixes: ["Episodes start faster: connecting no longer waits on peers that don't answer."],
  },
  {
    version: "0.4.0",
    date: "2026-10-04",
    features: [
      "New torrent engine: faster startup and much faster seeking, and seeking into parts that aren't downloaded yet no longer stalls. The installer is smaller too.",
      "Continue watching resumes instantly: the first seconds of an unfinished episode are kept, and Resume reopens the exact release you were watching.",
      "Download cache: played episodes stay on disk (10 GB by default, adjustable in Settings), so rewatching or seeking back doesn't download again.",
      "A loading screen while the app starts, and a new title bar.",
      "Settings → Help: Contact and Send logs, for reporting problems.",
    ],
    fixes: ["Subtitles show on the first play, and switching subtitles no longer re-buffers."],
  },
  {
    version: "0.3.2",
    date: "2026-09-29",
    features: [
      "The seek bar now shows which parts of the episode are already downloaded.",
      "New setting: Hardware video decoding (Settings → Playback). Turn it off if video turns blocky until you seek.",
      "A refreshed app icon and an animated loading screen while the app starts.",
      "This What's new window, after each update.",
    ],
    fixes: [
      "Episodes start much faster: the beginning and the part the player needs to open the file download first.",
      "Watching from the start downloads in order; resuming downloads from where you left off.",
      "Playback starts once about 10 seconds are buffered, so it stutters less.",
      "Seeking downloads the new spot first, right away.",
      "Fixed blocky or smeared video on freshly downloaded parts. If it still happens, the player repairs it on its own.",
      "Switching sources continues at the same time, without a false \"can't be played\" error.",
      "Slow peers no longer hold up the next bit of video.",
      "Long titles fit in the home banner.",
    ],
  },
];

/** Compares dotted versions ("0.3.10" > "0.3.2"). */
export function compareVersions(a: string, b: string): number {
  const pa = a.split(".").map(Number);
  const pb = b.split(".").map(Number);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const diff = (pa[i] ?? 0) - (pb[i] ?? 0);
    if (diff !== 0) return diff;
  }
  return 0;
}

/** Entries newer than `from`, up to and including `to`, newest first. */
export function changesBetween(from: string, to: string): ChangelogEntry[] {
  return CHANGELOG.filter((entry) => compareVersions(entry.version, from) > 0 && compareVersions(entry.version, to) <= 0);
}
