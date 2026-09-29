//! The Play page's feed: the SERVER INFO post and the Community News messages, mirrored
//! from the Echo VR Discord by the feed bot (`server/feed-bot`) into static files on
//! files.echovr.de. Images are referenced by content-hashed file names, so a name that
//! didn't change never needs downloading again.

use std::sync::OnceLock;

use anyhow::Result;
use serde::Deserialize;
use time::{OffsetDateTime, UtcOffset};

use crate::core::http;

pub const BASE: &str = "https://files.echovr.de/launcher/feed/";

/// The SERVER INFO message: its embeds, as Discord structures them.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Status {
    #[serde(default)]
    pub embeds: Vec<Embed>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Embed {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub fields: Vec<Field>,
    #[serde(default)]
    pub footer: Option<String>,
    /// RFC 3339; shown after the footer ("Today at 12:32 PM").
    #[serde(default)]
    pub timestamp: Option<String>,
    /// File name of the embed image (the world map) under [`BASE`].
    #[serde(default)]
    pub image: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Field {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub inline: bool,
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

pub fn fetch_status() -> Result<Status> {
    Ok(serde_json::from_str(&http::get_text(&format!(
        "{BASE}status.json"
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

pub fn mock_status() -> Status {
    let field = |name: &str, value: &str| Field {
        name: name.into(),
        value: value.into(),
        inline: true,
    };
    Status {
        embeds: vec![Embed {
            title: Some("Server Info".into()),
            description: Some(
                "-# Total | Public | Private\n\
                 - Online: `53` | `50` | `3`\n\
                 - Usage: `5%` | `6%` | `0%`\n\
                 - Status: `ok` | Startup: <t:1790410560:f>\n\
                 - Services: `If You Can Dodge a Wrench...`\n\
                 __**Region**__\n\
                 -# NA | EU | OCE\n\
                 - Online: `37` | `10` | `3`\n\
                 - Usage: `2%` | `20%` | `0%`\n\
                 __**Players**__\n\
                 - 30 Days: `4054`\n\
                 - 24 Hours: `843`\n\
                 - Last Hour: `35`\n\
                 - Online: `16`"
                    .into(),
            ),
            fields: vec![
                field("Lobby", "[7/12](https://echovr.de)"),
                field("Arena", "[8/16 (1)](https://echovr.de)"),
                field("Combat", "n/a"),
                field("Lobby Private", "n/a"),
                field("Arena Private", "[1/16](https://echovr.de)"),
                field("Combat Private", "n/a"),
            ],
            footer: Some("Last updated".into()),
            timestamp: Some(
                OffsetDateTime::now_utc()
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap_or_default(),
            ),
            image: None,
        }],
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
    fn parses_bot_output() {
        let s: Status = serde_json::from_str(
            r#"{"message":"1","edited_at":null,"content":"","embeds":[{"title":"Server Info",
            "description":"- Online: `53`","url":null,"color":5793266,"author":null,
            "fields":[{"name":"Lobby","value":"7/12","inline":true}],"footer":null,
            "timestamp":"2026-09-29T10:32:00+00:00","image":"map-0.1234abcd.png"}],
            "updated_at":"2026-09-29T10:32:05+00:00"}"#,
        )
        .unwrap();
        assert_eq!(s.embeds[0].fields[0].value, "7/12");
        assert_eq!(s.embeds[0].image.as_deref(), Some("map-0.1234abcd.png"));
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
