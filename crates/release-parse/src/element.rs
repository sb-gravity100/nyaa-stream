// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Port of anitopy's element.py (anitopy (c) Igor Cescon de Moura, itself a
// port of Anitomy (c) Eren Okka).

/// What a piece of a release title is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    AnimeSeason,
    AnimeSeasonPrefix,
    AnimeTitle,
    AnimeType,
    AnimeYear,
    AudioTerm,
    DeviceCompatibility,
    EpisodeNumber,
    EpisodeNumberAlt,
    EpisodePrefix,
    EpisodeTitle,
    FileChecksum,
    FileExtension,
    FileName,
    Language,
    Other,
    ReleaseGroup,
    ReleaseInformation,
    ReleaseVersion,
    Source,
    Subtitles,
    VideoResolution,
    VideoTerm,
    VolumeNumber,
    VolumePrefix,
    Unknown,
}

impl Category {
    /// anitopy's element name, e.g. `anime_title`.
    pub fn as_str(self) -> &'static str {
        use Category::*;
        match self {
            AnimeSeason => "anime_season",
            AnimeSeasonPrefix => "anime_season_prefix",
            AnimeTitle => "anime_title",
            AnimeType => "anime_type",
            AnimeYear => "anime_year",
            AudioTerm => "audio_term",
            DeviceCompatibility => "device_compatibility",
            EpisodeNumber => "episode_number",
            EpisodeNumberAlt => "episode_number_alt",
            EpisodePrefix => "episode_prefix",
            EpisodeTitle => "episode_title",
            FileChecksum => "file_checksum",
            FileExtension => "file_extension",
            FileName => "file_name",
            Language => "language",
            Other => "other",
            ReleaseGroup => "release_group",
            ReleaseInformation => "release_information",
            ReleaseVersion => "release_version",
            Source => "source",
            Subtitles => "subtitles",
            VideoResolution => "video_resolution",
            VideoTerm => "video_term",
            VolumeNumber => "volume_number",
            VolumePrefix => "volume_prefix",
            Unknown => "unknown",
        }
    }

    pub(crate) fn is_searchable(self) -> bool {
        use Category::*;
        matches!(
            self,
            AnimeSeasonPrefix
                | AnimeType
                | AudioTerm
                | DeviceCompatibility
                | EpisodePrefix
                | FileChecksum
                | Language
                | Other
                | ReleaseGroup
                | ReleaseInformation
                | ReleaseVersion
                | Source
                | Subtitles
                | VideoResolution
                | VideoTerm
                | VolumePrefix
        )
    }

    pub(crate) fn is_singular(self) -> bool {
        use Category::*;
        !matches!(
            self,
            AnimeSeason | AnimeType | AudioTerm | DeviceCompatibility | EpisodeNumber | Language | Other | ReleaseInformation | Source | VideoTerm
        )
    }
}

/// The parsed elements, in first-insertion order of their category (like
/// anitopy's dict).
#[derive(Clone, Debug, Default)]
pub struct Elements {
    items: Vec<(Category, Vec<String>)>,
    pub(crate) check_alt_number: bool,
}

impl Elements {
    pub(crate) fn insert(&mut self, category: Category, content: impl Into<String>) {
        let content = content.into();
        match self.items.iter_mut().find(|(c, _)| *c == category) {
            Some((_, values)) => values.push(content),
            None => self.items.push((category, vec![content])),
        }
    }

    pub(crate) fn erase(&mut self, category: Category) {
        self.items.retain(|(c, _)| *c != category);
    }

    pub(crate) fn remove(&mut self, category: Category, content: &str) {
        if let Some(pos) = self.items.iter().position(|(c, _)| *c == category) {
            let values = &mut self.items[pos].1;
            if let Some(i) = values.iter().position(|v| v == content) {
                values.remove(i);
            }
            if values.is_empty() {
                self.items.remove(pos);
            }
        }
    }

    pub fn contains(&self, category: Category) -> bool {
        self.items.iter().any(|(c, v)| *c == category && !v.is_empty())
    }

    /// Every value of a category (empty if none).
    pub fn get(&self, category: Category) -> &[String] {
        self.items.iter().find(|(c, _)| *c == category).map(|(_, v)| v.as_slice()).unwrap_or(&[])
    }

    pub fn first(&self, category: Category) -> Option<&str> {
        self.get(category).first().map(String::as_str)
    }

    pub fn iter(&self) -> impl Iterator<Item = (Category, &[String])> {
        self.items.iter().map(|(c, v)| (*c, v.as_slice()))
    }
}
