//! Batch self-verification (PLAN.md L1e): checks a nyaa batch release's
//! file list against the episode range its title names. A batch is verified
//! when its video episode files are exactly that run, each numbered once with
//! nothing unreadable; anything else comes back with the reasons.

use crate::files::is_video;
use crate::label::EpisodeLabel;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchIssue {
    /// The title is not a batch with an explicit episode range.
    NoTitleRange,
    /// Episodes inside the title range with no file.
    Missing(Vec<u32>),
    /// Episode files outside the title range.
    OutOfRange(Vec<u32>),
    /// Episode numbers that more than one file claims.
    Duplicates(Vec<u32>),
    /// Video files that are neither an episode nor an extra.
    Unread(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchCheck {
    pub verified: bool,
    /// Files numbered 1..=n for a title range a..=b with n == b-a+1 and a > 1
    /// ("S2 13-24" whose files say 01-12): verified, with offset a-1.
    pub offset: u32,
    pub issues: Vec<BatchIssue>,
}

/// Checks `file_paths` (folders joined by `/` or `\`) of the release `title`.
pub fn check_batch(title: &str, file_paths: &[&str]) -> BatchCheck {
    let (first, last, title_season, title_names_season) = match EpisodeLabel::of(title) {
        EpisodeLabel::Batch { episode_range: Some([a, b]), season, seasons, .. } if a <= b => (a, b, season, !seasons.is_empty()),
        label => {
            tracing::debug!(title, ?label, "batch check: title names no episode range");
            return BatchCheck { verified: false, offset: 0, issues: vec![BatchIssue::NoTitleRange] };
        }
    };

    // Episode numbers this title claims, and those of another season's files.
    let mut counted: Vec<u32> = Vec::new();
    let mut other_season: Vec<u32> = Vec::new();
    let mut unread = 0usize;
    for path in file_paths.iter().copied().filter(|path| is_video(path)) {
        match EpisodeLabel::of_path(path) {
            EpisodeLabel::Special => tracing::trace!(path, "batch check: extra, ignored"),
            EpisodeLabel::Episode { season, number, seasons } => {
                if title_names_season && !seasons.is_empty() && season != title_season {
                    tracing::trace!(path, season, number, title_season, "batch check: file is of another season");
                    other_season.push(number);
                } else {
                    tracing::trace!(path, number, "batch check: episode file counted");
                    counted.push(number);
                }
            }
            label => {
                tracing::trace!(path, ?label, "batch check: video file not read as an episode");
                unread += 1;
            }
        }
    }
    counted.sort_unstable();
    let mut duplicates: Vec<u32> = counted.windows(2).filter(|w| w[0] == w[1]).map(|w| w[0]).collect();
    duplicates.dedup();

    // Numbering the files actually use: the title's range, or 1..=n when a > 1
    // and every episode file sits in 1..=n (a season 2 pack numbered from 01).
    let n = last - first + 1;
    let offset_numbering = first > 1 && !counted.is_empty() && !counted.iter().any(|e| (first..=last).contains(e)) && counted.iter().all(|e| (1..=n).contains(e));
    let (lo, hi) = if offset_numbering { (1, n) } else { (first, last) };

    let clean = unread == 0 && other_season.is_empty();
    let verdict = if clean && counted.iter().copied().eq(first..=last) {
        Some(0)
    } else if clean && first > 1 && counted.iter().copied().eq(1..=n) {
        Some(first - 1)
    } else {
        None
    };

    let mut issues = Vec::new();
    if verdict.is_none() {
        let missing: Vec<u32> = (lo..=hi).filter(|e| !counted.contains(e)).collect();
        if !missing.is_empty() {
            issues.push(BatchIssue::Missing(missing));
        }
        let mut out: Vec<u32> = counted.iter().copied().filter(|e| !(lo..=hi).contains(e)).chain(other_season).collect();
        out.sort_unstable();
        out.dedup();
        if !out.is_empty() {
            issues.push(BatchIssue::OutOfRange(out));
        }
        if !duplicates.is_empty() {
            issues.push(BatchIssue::Duplicates(duplicates));
        }
        if unread > 0 {
            issues.push(BatchIssue::Unread(unread));
        }
    }

    let verified = verdict.is_some();
    let offset = verdict.unwrap_or(0);
    tracing::debug!(title, first, last, verified, offset, issues = issues.len(), "batch check verdict");
    BatchCheck { verified, offset, issues }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TITLE: &str = "[Erai-raws] Jujutsu Kaisen - 01 ~ 12 [1080p]";

    /// The 01~12 pack's episode files.
    fn pack() -> Vec<String> {
        (1..=12).map(|n| format!("[Erai-raws] Jujutsu Kaisen - {n:02} [1080p].mkv")).collect()
    }

    fn check(title: &str, files: &[String]) -> BatchCheck {
        let refs: Vec<&str> = files.iter().map(String::as_str).collect();
        check_batch(title, &refs)
    }

    fn verified() -> BatchCheck {
        BatchCheck { verified: true, offset: 0, issues: vec![] }
    }

    #[test]
    fn clean_pack_verifies() {
        assert_eq!(check(TITLE, &pack()), verified());
    }

    #[test]
    fn extras_in_an_nc_folder_are_ignored() {
        let mut files = pack();
        files.push("NC/[Erai-raws] Jujutsu Kaisen NCOP [1080p].mkv".to_string());
        files.push("NC/[Erai-raws] Jujutsu Kaisen NCED [1080p].mkv".to_string());
        assert_eq!(check(TITLE, &files), verified());
    }

    #[test]
    fn missing_episode_is_reported() {
        let mut files = pack();
        files.remove(6);
        assert_eq!(check(TITLE, &files), BatchCheck { verified: false, offset: 0, issues: vec![BatchIssue::Missing(vec![7])] });
    }

    #[test]
    fn duplicate_episode_is_reported() {
        let mut files = pack();
        files.push("[Erai-raws] Jujutsu Kaisen - 03 [1080p] [v2].mkv".to_string());
        assert_eq!(check(TITLE, &files), BatchCheck { verified: false, offset: 0, issues: vec![BatchIssue::Duplicates(vec![3])] });
    }

    #[test]
    fn unreadable_video_is_reported() {
        let mut files = pack();
        files.push("Jujutsu Kaisen Making Of.mkv".to_string());
        assert_eq!(check(TITLE, &files), BatchCheck { verified: false, offset: 0, issues: vec![BatchIssue::Unread(1)] });
    }

    #[test]
    fn season_two_range_with_files_numbered_from_one_verifies_with_offset() {
        let check = check("[Erai-raws] Jujutsu Kaisen 2nd Season - 13 ~ 24 [1080p]", &pack());
        assert_eq!(check, BatchCheck { verified: true, offset: 12, issues: vec![] });
    }

    #[test]
    fn file_of_another_season_is_out_of_range() {
        let mut files = pack();
        files.push("Season 1/[Erai-raws] Jujutsu Kaisen - 05 [1080p].mkv".to_string());
        let check = check("[Erai-raws] Jujutsu Kaisen 2nd Season - 13 ~ 24 [1080p]", &files);
        assert_eq!(check, BatchCheck { verified: false, offset: 0, issues: vec![BatchIssue::OutOfRange(vec![5])] });
    }

    #[test]
    fn non_batch_title_has_no_range() {
        let check = check("[SubsPlease] Jujutsu Kaisen - 05 (1080p)", &pack());
        assert_eq!(check, BatchCheck { verified: false, offset: 0, issues: vec![BatchIssue::NoTitleRange] });
    }

    #[test]
    fn non_video_files_are_ignored() {
        let mut files = pack();
        files.push("Subs/[Erai-raws] Jujutsu Kaisen - 01.ass".to_string());
        files.push("Artwork/cover.jpg".to_string());
        assert_eq!(check(TITLE, &files), verified());
    }
}
