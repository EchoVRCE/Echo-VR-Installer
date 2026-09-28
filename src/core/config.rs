//! `~/.echovr_installer/paths.properties` -- the same file and key (`pc.install.path`) the
//! Java installer used, so a saved path carries over.

use std::path::PathBuf;

const KEY: &str = "pc.install.path";

fn file() -> Option<PathBuf> {
    Some(
        dirs::home_dir()?
            .join(".echovr_installer")
            .join("paths.properties"),
    )
}

/// Minimal `java.util.Properties` reader: `key=value` / `key: value`, `#`/`!` comments,
/// and the backslash escapes `Properties.store` writes (notably `\:` and `\\`).
fn parse(text: &str) -> Vec<(String, String)> {
    text.lines()
        .map(str::trim_start)
        .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with('!'))
        .filter_map(|l| {
            let idx = l.find(['=', ':'])?;
            Some((
                unescape(l[..idx].trim()),
                unescape(l[idx + 1..].trim_start()),
            ))
        })
        .collect()
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('t') => out.push('\t'),
                Some('n') => out.push('\n'),
                Some(other) => out.push(other),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

pub fn load_install_path() -> Option<String> {
    let text = std::fs::read_to_string(file()?).ok()?;
    parse(&text)
        .into_iter()
        .find(|(k, _)| k == KEY)
        .map(|(_, v)| v)
        .filter(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_java_properties_output() {
        let text = "#Echo VR Installer saved paths\n#Tue Sep 23 11:00:00 CEST 2026\npc.install.path=C\\:/Program Files/Oculus/Software/Software\n";
        let e = parse(text);
        assert_eq!(
            e,
            vec![(
                KEY.to_string(),
                "C:/Program Files/Oculus/Software/Software".to_string()
            )]
        );
    }

    #[test]
    fn unescapes_java_escapes() {
        assert_eq!(unescape("C\\:/a\\=b\\#c\\\\d"), "C:/a=b#c\\d");
    }
}
