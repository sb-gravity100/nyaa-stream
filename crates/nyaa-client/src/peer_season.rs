//! Peer-consistent season for unmarked releases (PLAN.md L1c).
//!
//! Many nyaa titles carry no season marker ("[Group] Show - 09"), so the title
//! alone cannot say which season they belong to. Other fansub groups that post
//! marked releases ("[Group] Show S2 - 03") of the same show during the same
//! airing run give the season: an unmarked release takes the season that the
//! marked releases of the show use in its time window. Grouping is by show stem
//! only; the releasing group is not part of the key, because a group's own
//! marking template is consistent (one group always marks, another never does),
//! so only other groups' marked releases carry the signal.

use std::collections::{HashMap, HashSet};

use tracing::{debug, trace};

use crate::label::EpisodeLabel;
use crate::stem::parse_stem;

/// Days either side of a season's marked releases that count as its window.
const WINDOW_DAYS: i64 = 14;

/// Marked releases of one season needed before they form a window.
const MIN_MARKED: usize = 3;

/// Distinct groups (an empty group counts as its own "(none)" group) among a
/// season's marked releases needed before they form a window.
const MIN_GROUPS: usize = 2;

/// An unmarked episode may be this many numbers past the highest marked episode
/// of its window's season; beyond that it is taken as absolute numbering.
const EPISODE_SLACK: u32 = 3;

/// One release as input: its title and publish time ("YYYY-MM-DD HH:MM"; only the first 10 chars are used).
pub struct PeerRelease<'a> {
    pub title: &'a str,
    pub published: &'a str,
}

/// Days since 1970-01-01 of a proleptic Gregorian date (Howard Hinnant's days_from_civil).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The day number of the first 10 chars of `published` ("YYYY-MM-DD"), or `None` when unparsable.
fn day_number(published: &str) -> Option<i64> {
    let s = published.get(..10)?;
    if s.get(4..5)? != "-" || s.get(7..8)? != "-" {
        trace!(published, "peer season: date separators missing");
        return None;
    }
    let year: i64 = s.get(0..4)?.parse().ok()?;
    let month: i64 = s.get(5..7)?.parse().ok()?;
    let day: i64 = s.get(8..10)?.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        trace!(published, "peer season: date out of range");
        return None;
    }
    Some(days_from_civil(year, month, day))
}

/// How a release takes part in the pass. `episode` is `Some` for an Episode label, `None` for a Batch.
enum Role {
    /// Names exactly one season.
    Marked { season: u32, episode: Option<u32> },
    /// Names no season.
    Unmarked { episode: Option<u32> },
}

fn role_of(title: &str) -> Option<Role> {
    match EpisodeLabel::of(title) {
        EpisodeLabel::Episode { number, seasons, .. } => match seasons.as_slice() {
            [] => Some(Role::Unmarked { episode: Some(number) }),
            [season] => Some(Role::Marked { season: *season, episode: Some(number) }),
            // Multi-season packs do not vote.
            _ => None,
        },
        EpisodeLabel::Batch { seasons, .. } => match seasons.as_slice() {
            [] => Some(Role::Unmarked { episode: None }),
            [season] => Some(Role::Marked { season: *season, episode: None }),
            _ => None,
        },
        _ => None,
    }
}

/// A marked release of a show: its day, season, group ("(none)" when empty) and episode (`None` for a batch).
struct Marked {
    day: i64,
    season: u32,
    group: String,
    episode: Option<u32>,
}

/// An unmarked release of a show: its input index, day and episode (`None` for a batch).
struct Unmarked {
    index: usize,
    day: i64,
    episode: Option<u32>,
}

/// One season's window over a show's marked releases.
struct Window {
    season: u32,
    lo: i64,
    hi: i64,
    /// Highest episode number among that season's marked episodes, when there are any.
    max_episode: Option<u32>,
}

/// For every input, `Some(season)` when the release is unmarked and its peers give it a season, else `None`
/// (marked releases always get `None`: their own marker stands). Output has the same length and order as input.
///
/// Releases are grouped by show stem, ignoring the releasing group. A season forms a window only when its
/// marked releases come from at least `MIN_GROUPS` distinct groups and number at least `MIN_MARKED`; the window
/// is the span of those releases padded by `WINDOW_DAYS`. An unmarked release takes a season when it falls in
/// exactly one window and, if it is an episode, its number is at most `EPISODE_SLACK` past that season's highest
/// marked episode. Batches skip the episode check.
pub fn peer_seasons(releases: &[PeerRelease]) -> Vec<Option<u32>> {
    let mut out: Vec<Option<u32>> = vec![None; releases.len()];

    // stem -> (marked releases, unmarked releases)
    let mut keys: HashMap<String, (Vec<Marked>, Vec<Unmarked>)> = HashMap::new();
    let mut skipped = 0usize;
    for (index, release) in releases.iter().enumerate() {
        let Some(role) = role_of(release.title) else {
            continue;
        };
        let Some((stem, group)) = parse_stem(release.title) else {
            trace!(index, title = release.title, "peer season: no stem, skipped");
            skipped += 1;
            continue;
        };
        let Some(day) = day_number(release.published) else {
            trace!(index, title = release.title, "peer season: unparsable date, skipped");
            skipped += 1;
            continue;
        };
        let entry = keys.entry(stem).or_default();
        match role {
            Role::Marked { season, episode } => {
                let group = if group.is_empty() { "(none)".to_string() } else { group.to_lowercase() };
                entry.0.push(Marked { day, season, group, episode });
            }
            Role::Unmarked { episode } => entry.1.push(Unmarked { index, day, episode }),
        }
    }

    let key_count = keys.len();
    let mut propagated = 0usize;
    let mut sanity_rejected = 0usize;
    for (stem, (marked, unmarked)) in keys {
        if marked.is_empty() || unmarked.is_empty() {
            continue;
        }
        let mut seasons: Vec<u32> = marked.iter().map(|m| m.season).collect();
        seasons.sort_unstable();
        seasons.dedup();
        let mut windows: Vec<Window> = Vec::new();
        for season in seasons {
            let members: Vec<&Marked> = marked.iter().filter(|m| m.season == season).collect();
            let groups: HashSet<&str> = members.iter().map(|m| m.group.as_str()).collect();
            if members.len() < MIN_MARKED || groups.len() < MIN_GROUPS {
                trace!(stem = stem.as_str(), season, count = members.len(), groups = groups.len(), "peer season: no window for this season");
                continue;
            }
            let earliest = members.iter().map(|m| m.day).min().unwrap_or_default();
            let latest = members.iter().map(|m| m.day).max().unwrap_or_default();
            let max_episode = members.iter().filter_map(|m| m.episode).max();
            windows.push(Window { season, lo: earliest - WINDOW_DAYS, hi: latest + WINDOW_DAYS, max_episode });
        }
        if windows.is_empty() {
            continue;
        }
        for release in unmarked {
            let hits: Vec<&Window> = windows.iter().filter(|w| (w.lo..=w.hi).contains(&release.day)).collect();
            let window = match hits.as_slice() {
                [only] => *only,
                _ => {
                    trace!(title = releases[release.index].title, hits = hits.len(), "peer season: not in exactly one window, left unset");
                    continue;
                }
            };
            if let (Some(number), Some(max_episode)) = (release.episode, window.max_episode) {
                if number > max_episode + EPISODE_SLACK {
                    trace!(title = releases[release.index].title, number, max_episode, season = window.season, "peer season: episode number too far past the season's episodes, left unset");
                    sanity_rejected += 1;
                    continue;
                }
            }
            out[release.index] = Some(window.season);
            propagated += 1;
            trace!(title = releases[release.index].title, season = window.season, "peer season: propagated");
        }
    }

    debug!(inputs = releases.len(), keys = key_count, skipped, sanity_rejected, propagated, "peer season pass done");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rel<'a>(title: &'a str, published: &'a str) -> PeerRelease<'a> {
        PeerRelease { title, published }
    }

    #[test]
    fn day_numbers_count_from_the_epoch() {
        assert_eq!(day_number("1970-01-01 00:00"), Some(0));
        assert_eq!(day_number("1970-01-02"), Some(1));
        assert_eq!(day_number("2024-02-29 12:00"), Some(19_782));
        assert_eq!(day_number("2024-13-01"), None);
        assert_eq!(day_number("garbage"), None);
    }

    #[test]
    fn unmarked_release_takes_the_season_of_its_window_across_groups() {
        let releases = [
            rel("[Grp] Show S2 - 01 [1080p]", "2024-04-07 10:00"),
            rel("[Other] Show S2 - 02 [1080p]", "2024-04-14 10:00"),
            rel("[Grp] Show S2 - 03 [1080p]", "2024-04-21 10:00"),
            rel("[Third] Show - 04 [1080p]", "2024-04-28 10:00"),
        ];
        assert_eq!(peer_seasons(&releases), vec![None, None, None, Some(2)]);
    }

    #[test]
    fn single_group_marked_releases_form_no_window() {
        let releases = [
            rel("[Grp] Show S2 - 01 [1080p]", "2024-04-07 10:00"),
            rel("[Grp] Show S2 - 02 [1080p]", "2024-04-14 10:00"),
            rel("[Grp] Show S2 - 03 [1080p]", "2024-04-21 10:00"),
            rel("[Other] Show - 04 [1080p]", "2024-04-28 10:00"),
        ];
        assert_eq!(peer_seasons(&releases)[3], None);
    }

    #[test]
    fn too_few_marked_releases_form_no_window() {
        let releases = [
            rel("[Grp] Show S2 - 01 [1080p]", "2024-04-07 10:00"),
            rel("[Other] Show S2 - 02 [1080p]", "2024-04-14 10:00"),
            rel("[Third] Show - 03 [1080p]", "2024-04-21 10:00"),
        ];
        assert_eq!(peer_seasons(&releases)[2], None);
    }

    #[test]
    fn unmarked_release_outside_every_window_gets_none() {
        let releases = [
            rel("[Grp] Show S2 - 01 [1080p]", "2024-04-07 10:00"),
            rel("[Other] Show S2 - 02 [1080p]", "2024-04-14 10:00"),
            rel("[Grp] Show S2 - 03 [1080p]", "2024-04-21 10:00"),
            rel("[Third] Show - 04 [1080p]", "2023-01-01 10:00"),
        ];
        assert_eq!(peer_seasons(&releases)[3], None);
    }

    #[test]
    fn empty_group_counts_as_its_own_group() {
        let releases = [
            rel("Show S2 - 01 [1080p]", "2024-04-07 10:00"),
            rel("[Grp] Show S2 - 02 [1080p]", "2024-04-14 10:00"),
            rel("[Grp] Show S2 - 03 [1080p]", "2024-04-21 10:00"),
            rel("[Third] Show - 04 [1080p]", "2024-04-28 10:00"),
        ];
        assert_eq!(peer_seasons(&releases)[3], Some(2));
    }

    #[test]
    fn one_marked_release_forms_no_window() {
        let releases = [rel("[Grp] Show S2 - 01 [1080p]", "2024-04-07 10:00"), rel("[Other] Show - 03 [1080p]", "2024-04-21 10:00")];
        assert_eq!(peer_seasons(&releases), vec![None, None]);
    }

    #[test]
    fn overlapping_season_windows_leave_the_release_unset() {
        let releases = [
            rel("[A] Show S1 - 01 [1080p]", "2024-04-07 10:00"),
            rel("[B] Show S1 - 02 [1080p]", "2024-04-14 10:00"),
            rel("[A] Show S1 - 03 [1080p]", "2024-04-21 10:00"),
            rel("[A] Show S2 - 01 [1080p]", "2024-04-10 10:00"),
            rel("[B] Show S2 - 02 [1080p]", "2024-04-17 10:00"),
            rel("[A] Show S2 - 03 [1080p]", "2024-04-24 10:00"),
            rel("[C] Show - 04 [1080p]", "2024-05-01 10:00"),
        ];
        assert_eq!(peer_seasons(&releases)[6], None);
    }

    #[test]
    fn episode_number_far_past_the_season_is_rejected() {
        let releases = [
            rel("[Grp] Show S2 - 01 [1080p]", "2024-04-07 10:00"),
            rel("[Other] Show S2 - 02 [1080p]", "2024-04-14 10:00"),
            rel("[Grp] Show S2 - 03 [1080p]", "2024-04-21 10:00"),
            rel("[Third] Show - 13 [1080p]", "2024-04-28 10:00"),
        ];
        // n = m + 10 with m = 3: taken as absolute numbering.
        assert_eq!(peer_seasons(&releases)[3], None);
    }

    #[test]
    fn episode_number_within_slack_is_kept() {
        let releases = [
            rel("[Grp] Show S2 - 01 [1080p]", "2024-04-07 10:00"),
            rel("[Other] Show S2 - 02 [1080p]", "2024-04-14 10:00"),
            rel("[Grp] Show S2 - 03 [1080p]", "2024-04-21 10:00"),
            rel("[Third] Show - 06 [1080p]", "2024-04-28 10:00"),
        ];
        assert_eq!(peer_seasons(&releases)[3], Some(2));
    }

    #[test]
    fn output_matches_input_length_and_order() {
        let releases = [
            rel("[Grp] Show S2 - 01 [1080p]", "2024-04-07 10:00"),
            rel("not a release title at all", "2024-04-07 10:00"),
            rel("[Other] Show S2 - 02 [1080p]", "not-a-date"),
            rel("[Other] Show S2 - 03 [1080p]", "2024-04-14 10:00"),
            rel("[Grp] Show S2 - 04 [1080p]", "2024-04-21 10:00"),
            rel("[Third] Show - 05 [1080p]", "2024-04-28 10:00"),
        ];
        let out = peer_seasons(&releases);
        assert_eq!(out.len(), releases.len());
        assert_eq!(out[5], Some(2));
        assert_eq!(peer_seasons(&[]).len(), 0);
    }
}
