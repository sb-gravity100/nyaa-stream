import type { NyaaResult } from "./types";

// Most seeders wins - fastest/most reliable swarm. Used both to auto-pick
// the initial source for an episode's play button and to order the
// player's manual source-picker dropdown.
export function bestRelease(releases: NyaaResult[]): NyaaResult {
  return releases.reduce((best, r) => (r.seeders > best.seeders ? r : best));
}
