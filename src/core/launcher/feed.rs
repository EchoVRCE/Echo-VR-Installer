//! The Play page's feed, from static files on files.echovr.de: SERVER INFO
//! (`servers.json`, aggregated from the EchoVRCE status API by `server/feed-bot/
//! status_feed.py`) and Community News (`news.json`, mirrored from Discord by the feed bot).
//! News images are referenced by content-hashed file names, so a name that didn't change
//! never needs downloading again.

use std::sync::OnceLock;

use anyhow::Result;
use serde::Deserialize;
use time::{OffsetDateTime, UtcOffset};

use crate::core::http;

pub const BASE: &str = "https://files.echovr.de/launcher/feed/";

/// SERVER INFO: servers, how busy they are, players and matches.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Servers {
    /// "ok", "stale" (the API's data is old) or "down" (the API doesn't answer).
    #[serde(default)]
    pub status: String,
    /// When the game servers' backend started (RFC 3339).
    #[serde(default)]
    pub started_at: Option<String>,
    /// When this was written (RFC 3339).
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub servers: Split,
    /// Percent of servers running a match.
    #[serde(default)]
    pub usage: Split,
    #[serde(default)]
    pub regions: Regions,
    #[serde(default)]
    pub players: Players,
    #[serde(default)]
    pub modes: Modes,
    #[serde(default)]
    pub locations: Vec<Location>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct Split {
    #[serde(default)]
    pub total: u32,
    #[serde(default)]
    pub public: u32,
    #[serde(default)]
    pub private: u32,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub struct Regions {
    #[serde(default)]
    pub na: Region,
    #[serde(default)]
    pub eu: Region,
    #[serde(default)]
    pub oce: Region,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct Region {
    #[serde(default)]
    pub servers: u32,
    #[serde(default)]
    pub usage: u32,
}

/// Different players seen over time, and online now.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Players {
    #[serde(default)]
    pub online: u32,
    #[serde(default)]
    pub last_hour: u32,
    #[serde(default)]
    pub last_24h: u32,
    #[serde(default)]
    pub last_30d: u32,
    /// Since when players are counted (RFC 3339).
    #[serde(default)]
    pub since: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Modes {
    #[serde(default)]
    pub lobby: Mode,
    #[serde(default)]
    pub arena: Mode,
    #[serde(default)]
    pub combat: Mode,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct Mode {
    #[serde(default)]
    pub public: Slot,
    #[serde(default)]
    pub private: Slot,
}

/// The matches of one mode and kind, added up.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct Slot {
    #[serde(default)]
    pub matches: u32,
    #[serde(default)]
    pub players: u32,
    #[serde(default)]
    pub limit: u32,
    #[serde(default)]
    pub spectators: u32,
}

/// Servers in one place, for the map.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Location {
    #[serde(default)]
    pub lat: f32,
    #[serde(default)]
    pub lon: f32,
    #[serde(default)]
    pub servers: u32,
    #[serde(default)]
    pub matches: u32,
}

/// The Community News blocks: `main` feeds the banner and the first card, `community`
/// the second card. Either may be unset.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct News {
    #[serde(default)]
    pub slots: Slots,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Slots {
    #[serde(default)]
    pub main: Option<NewsItem>,
    #[serde(default)]
    pub community: Option<NewsItem>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct NewsItem {
    #[serde(default)]
    pub title: String,
    /// Discord markdown.
    #[serde(default)]
    pub body: String,
    /// File name of the message's first image under [`BASE`].
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub link_label: String,
    #[serde(default)]
    pub link_url: String,
}

pub fn fetch_servers() -> Result<Servers> {
    Ok(serde_json::from_str(&http::get_text(&format!(
        "{BASE}servers.json"
    ))?)?)
}

pub fn fetch_news() -> Result<News> {
    Ok(serde_json::from_str(&http::get_text(&format!(
        "{BASE}news.json"
    ))?)?)
}

/// A feed image by file name.
pub fn fetch_image(name: &str) -> Result<image::RgbaImage> {
    let bytes = http::get_bytes(&format!("{BASE}{name}"))?;
    Ok(image::load_from_memory(&bytes)?.to_rgba8())
}

// ---- time ----

static LOCAL_OFFSET: OnceLock<UtcOffset> = OnceLock::new();

/// Reads the local UTC offset. Call first thing in `main`: on Unix the `time` crate only
/// answers while the process is still single-threaded.
pub fn init_local_offset() {
    if let Ok(o) = UtcOffset::current_local_offset() {
        let _ = LOCAL_OFFSET.set(o);
    }
}

fn local(t: OffsetDateTime) -> OffsetDateTime {
    t.to_offset(LOCAL_OFFSET.get().copied().unwrap_or(UtcOffset::UTC))
}

fn now_local() -> OffsetDateTime {
    local(OffsetDateTime::now_utc())
}

pub fn parse_time(s: &str) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339).ok()
}

fn clock(t: OffsetDateTime) -> String {
    let (h, m) = (t.hour(), t.minute());
    let h12 = if h % 12 == 0 { 12 } else { h % 12 };
    format!("{h12}:{m:02} {}", if h < 12 { "AM" } else { "PM" })
}

fn month(t: OffsetDateTime) -> String {
    t.month().to_string()
}

/// Discord's footer time: "Today at 12:32 PM", "Yesterday at …", else "9/26/2026 8:16 AM".
pub fn footer_time(t: OffsetDateTime) -> String {
    footer_time_at(local(t), now_local())
}

fn footer_time_at(t: OffsetDateTime, now: OffsetDateTime) -> String {
    let days = (now.date() - t.date()).whole_days();
    match days {
        0 => format!("Today at {}", clock(t)),
        1 => format!("Yesterday at {}", clock(t)),
        _ => format!(
            "{}/{}/{} {}",
            u8::from(t.month()),
            t.day(),
            t.year(),
            clock(t)
        ),
    }
}

/// A Discord timestamp mention (`<t:unix:style>`) in local time.
pub fn discord_time(unix: i64, style: char) -> String {
    let Ok(t) = OffsetDateTime::from_unix_timestamp(unix) else {
        return String::new();
    };
    discord_time_at(local(t), style, now_local())
}

fn discord_time_at(t: OffsetDateTime, style: char, now: OffsetDateTime) -> String {
    let date_long = format!("{} {}, {}", month(t), t.day(), t.year());
    match style {
        't' => clock(t),
        'T' => {
            let s = clock(t);
            let (hm, ampm) = s.split_once(' ').unwrap_or((&s, ""));
            format!("{hm}:{:02} {ampm}", t.second())
        }
        'd' => format!("{}/{}/{}", u8::from(t.month()), t.day(), t.year()),
        'D' => date_long,
        'F' => format!("{}, {date_long} {}", t.weekday(), clock(t)),
        'R' => {
            let secs = (now - t).whole_seconds();
            let (n, unit) = match secs.unsigned_abs() {
                s if s < 60 => (s, "second"),
                s if s < 3600 => (s / 60, "minute"),
                s if s < 86_400 => (s / 3600, "hour"),
                s if s < 30 * 86_400 => (s / 86_400, "day"),
                s if s < 365 * 86_400 => (s / (30 * 86_400), "month"),
                s => (s / (365 * 86_400), "year"),
            };
            let plural = if n == 1 { "" } else { "s" };
            if secs >= 0 {
                format!("{n} {unit}{plural} ago")
            } else {
                format!("in {n} {unit}{plural}")
            }
        }
        // 'f', the default.
        _ => format!("{date_long} {}", clock(t)),
    }
}

// ---- made-up data (snapshots, and the look of the design concept) ----

/// The design concept's numbers.
pub fn mock_servers() -> Servers {
    let slot = |matches, players, limit, spectators| Slot {
        matches,
        players,
        limit,
        spectators,
    };
    let now = OffsetDateTime::now_utc();
    let rfc = |t: OffsetDateTime| {
        t.format(&time::format_description::well_known::Rfc3339)
            .ok()
    };
    let at = |lat, lon, servers, matches| Location {
        lat,
        lon,
        servers,
        matches,
    };
    Servers {
        status: "ok".into(),
        started_at: OffsetDateTime::from_unix_timestamp(1_790_410_560)
            .ok()
            .and_then(rfc),
        updated_at: rfc(now),
        servers: Split {
            total: 53,
            public: 50,
            private: 3,
        },
        usage: Split {
            total: 5,
            public: 6,
            private: 0,
        },
        regions: Regions {
            na: Region {
                servers: 37,
                usage: 2,
            },
            eu: Region {
                servers: 10,
                usage: 20,
            },
            oce: Region {
                servers: 3,
                usage: 0,
            },
        },
        players: Players {
            online: 16,
            last_hour: 35,
            last_24h: 843,
            last_30d: 4054,
            since: rfc(now - time::Duration::days(60)),
        },
        modes: Modes {
            lobby: Mode {
                public: slot(1, 7, 12, 0),
                private: Slot::default(),
            },
            arena: Mode {
                public: slot(1, 8, 16, 1),
                private: slot(1, 1, 16, 0),
            },
            combat: Mode::default(),
        },
        locations: vec![
            at(41.9, -87.6, 8, 1),
            at(40.7, -74.2, 3, 0),
            at(32.8, -96.8, 7, 1),
            at(39.7, -105.0, 3, 0),
            at(51.5, -0.1, 5, 1),
            at(51.0, 11.0, 2, 0),
            at(-27.5, 153.0, 3, 0),
        ],
    }
}

pub fn mock_news() -> News {
    News {
        slots: Slots {
            main: Some(NewsItem {
                title: "Title".into(),
                body: "Iusto odio ducimus qui blanditiis praesentium voluptatum deleniti atque\n\n\
                       - Quos dolores et quas\n\
                       - Molestias excepturi sint occaecati\n\
                       - Cupiditate non provident.\n\n\
                       Similique sunt in culpa qui officia deserunt mollitia animi, id"
                    .into(),
                image: None,
                link_label: "How to play".into(),
                link_url: "https://echovr.de".into(),
            }),
            community: Some(NewsItem {
                title: "Community".into(),
                body: "Iusto odio dignissimos ducimus qui  voluptatum deleniti atque corrupti\n\n\
                       Similique sunt in culpa qui officia deserunt mollitia est laborum et \
                       dolorum fuga. Et harum quidem"
                    .into(),
                image: None,
                link_label: "Link".into(),
                link_url: "https://echovr.de".into(),
            }),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> OffsetDateTime {
        parse_time(s).unwrap()
    }

    #[test]
    fn parses_feed_files() {
        let s: Servers = serde_json::from_str(
            r#"{"status":"ok","source_time":"2026-09-29T11:36:32Z",
            "started_at":"2026-09-28T17:50:32+00:00",
            "servers":{"total":62,"public":52,"private":10},
            "usage":{"total":2,"public":2,"private":0},
            "regions":{"NA":{"servers":49,"usage":2},"EU":{"servers":10,"usage":0},
            "OCE":{"servers":3,"usage":0}},
            "players":{"online":4,"last_hour":4,"last_24h":4,"last_30d":4,
            "since":"2026-09-29T11:36:33+00:00"},
            "modes":{"lobby":{"public":{"matches":1,"players":4,"limit":12,"spectators":0},
            "private":{"matches":0,"players":0,"limit":0,"spectators":0}}},
            "locations":[{"region":"us-ne","name":"NE","country":"US","lat":41.26,
            "lon":-95.94,"servers":7,"matches":1}],
            "updated_at":"2026-09-29T11:36:33+00:00"}"#,
        )
        .unwrap();
        assert_eq!(s.servers.public, 52);
        assert_eq!(s.regions.na.servers, 49);
        assert_eq!(s.modes.lobby.public.limit, 12);
        assert_eq!(s.modes.combat.public.matches, 0);
        assert_eq!(s.locations[0].matches, 1);
        let n: News = serde_json::from_str(
            r#"{"slots":{"main":{"id":"1","title":"Halloween","body":"- a","image":null,
            "link_label":"READ MORE","link_url":"https://x","jump_url":"https://y",
            "posted_at":null,"edited_at":null},"community":null},"updated_at":"x"}"#,
        )
        .unwrap();
        assert_eq!(n.slots.main.unwrap().title, "Halloween");
        assert!(n.slots.community.is_none());
    }

    #[test]
    fn footer_times() {
        let now = at("2026-09-29T15:00:00Z");
        assert_eq!(
            footer_time_at(at("2026-09-29T12:32:00Z"), now),
            "Today at 12:32 PM"
        );
        assert_eq!(
            footer_time_at(at("2026-09-28T00:05:00Z"), now),
            "Yesterday at 12:05 AM"
        );
        assert_eq!(
            footer_time_at(at("2026-09-26T08:16:00Z"), now),
            "9/26/2026 8:16 AM"
        );
    }

    #[test]
    fn discord_timestamps() {
        let t = at("2026-09-26T08:16:00Z");
        let now = at("2026-09-29T08:16:00Z");
        assert_eq!(discord_time_at(t, 'f', now), "September 26, 2026 8:16 AM");
        assert_eq!(discord_time_at(t, 't', now), "8:16 AM");
        assert_eq!(discord_time_at(t, 'R', now), "3 days ago");
        assert_eq!(discord_time_at(now, 'R', t), "in 3 days");
    }
}
