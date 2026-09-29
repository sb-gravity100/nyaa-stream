//! Kitsu as the metadata fallback when AniList is unavailable - rate
//! limited (429), erroring (5xx) or unreachable. Kitsu results carry the
//! AniList id they map to, so everything downstream (routes, library,
//! watch progress) keeps working off AniList ids.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use anilist_client::{AnimeMedia, AnimeTitle, CoverImage};
use kitsu_client::KitsuAnime;

/// After AniList turns us away, go straight to Kitsu for this long instead
/// of hitting AniList again (which only extends a rate limit).
const COOLDOWN: Duration = Duration::from_secs(60);

#[derive(Default)]
pub struct AniListCooldown {
    until: Mutex<Option<Instant>>,
}

impl AniListCooldown {
    /// Whether AniList should be skipped right now.
    pub fn active(&self) -> bool {
        self.until.lock().map(|until| until.is_some_and(|t| Instant::now() < t)).unwrap_or(false)
    }

    pub fn start(&self) {
        tracing::warn!(seconds = COOLDOWN.as_secs(), "AniList unavailable, using Kitsu for a while");
        if let Ok(mut until) = self.until.lock() {
            *until = Some(Instant::now() + COOLDOWN);
        }
    }
}

/// Rate limited, server error, or couldn't reach AniList at all - as
/// opposed to a bad request, which Kitsu wouldn't fix.
pub fn anilist_unavailable(err: &anyhow::Error) -> bool {
    let Some(err) = err.downcast_ref::<reqwest::Error>() else {
        return false;
    };
    match err.status() {
        Some(status) => status.as_u16() == 429 || status.is_server_error(),
        None => err.is_connect() || err.is_timeout() || err.is_request(),
    }
}

/// Kitsu's shape in AniList's terms.
pub fn to_media(anime: KitsuAnime) -> AnimeMedia {
    let (season, season_year) = anime
        .start_date
        .as_deref()
        .and_then(|date| {
            let mut parts = date.split('-');
            let year: i32 = parts.next()?.parse().ok()?;
            let month: u32 = parts.next()?.parse().ok()?;
            let season = match month {
                1..=3 => "WINTER",
                4..=6 => "SPRING",
                7..=9 => "SUMMER",
                _ => "FALL",
            };
            Some((Some(season.to_string()), Some(year)))
        })
        .unwrap_or((None, None));
    let format = anime.subtype.as_deref().map(|subtype| {
        match subtype {
            "movie" => "MOVIE",
            "special" => "SPECIAL",
            "music" => "MUSIC",
            // TV, OVA, ONA are already AniList's spelling.
            other => return other.to_uppercase(),
        }
        .to_string()
    });
    AnimeMedia {
        id: anime.anilist_id,
        title: AnimeTitle {
            romaji: anime.title_romaji.or(anime.canonical_title),
            english: anime.title_english,
            native: anime.title_native,
        },
        description: anime.synopsis,
        episodes: anime.episode_count,
        cover_image: CoverImage { large: anime.poster_large, extra_large: anime.poster_original },
        average_score: anime.average_rating.map(|rating| rating.round() as i32),
        format,
        season,
        season_year,
        duration: anime.episode_length,
        status: None,
        synonyms: Vec::new(),
        next_airing_episode: None,
    }
}
