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
