use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub body: String,
    pub source: String,
}

const BUILTIN: &[(&str, &str)] = &[
    ("recon", include_str!("../../skills/recon.md")),
    ("web", include_str!("../../skills/web.md")),
    ("api", include_str!("../../skills/api.md")),
    ("ad", include_str!("../../skills/ad.md")),
    ("creds", include_str!("../../skills/creds.md")),
    ("linux-privesc", include_str!("../../skills/linux-privesc.md")),
    ("windows-privesc", include_str!("../../skills/windows-privesc.md")),
    ("pivot", include_str!("../../skills/pivot.md")),
    ("payloads", include_str!("../../skills/payloads.md")),
    ("cloud", include_str!("../../skills/cloud.md")),
    ("k8s", include_str!("../../skills/k8s.md")),
    ("wireless", include_str!("../../skills/wireless.md")),
    ("forensics", include_str!("../../skills/forensics.md")),
    ("osint", include_str!("../../skills/osint.md")),
    ("tricks", include_str!("../../skills/tricks.md")),
];

fn parse(text: &str, source: &str, fallback: &str) -> Skill {
    let (fm, body) = if let Some(rest) = text.strip_prefix("---\n") {
        match rest.split_once("\n---\n") {
            Some((fm, body)) => (fm, body),
            None => ("", text),
        }
    } else {
        ("", text)
    };

    let mut name = fallback.to_string();
    let mut description = String::new();
    let mut tags: Vec<String> = Vec::new();

    for line in fm.lines() {
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        let v = v.trim();
        match k.trim() {
            "name" => name = v.trim_matches('"').to_string(),
            "description" => description = v.trim_matches('"').to_string(),
            "tags" => {
                tags = v
                    .trim_matches(['[', ']'])
                    .split(',')
                    .map(|t| t.trim().trim_matches('"').to_string())
                    .filter(|t| !t.is_empty())
                    .collect()
            }
            _ => {}
        }
    }

    Skill {
        name,
        description,
        tags,
        body: body.trim().to_string(),
        source: source.to_string(),
    }
}

fn search_dirs() -> Vec<(PathBuf, &'static str)> {
    vec![
        (crate::config::dir().join("skills"), "user"),
        (PathBuf::from(".abyss/skills"), "project"),
    ]
}

/// Builtin skills, then user (~/.abyss/skills) and project (./.abyss/skills) overrides by name.
pub fn all() -> Vec<Skill> {
    let mut map: BTreeMap<String, Skill> = BTreeMap::new();

    for (fallback, text) in BUILTIN {
        let s = parse(text, "builtin", fallback);
        map.insert(s.name.clone(), s);
    }

    for (dir, src) in search_dirs() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut paths: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
        paths.sort();
        for p in paths {
            if p.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&p) else {
                continue;
            };
            let fallback = p.file_stem().and_then(|s| s.to_str()).unwrap_or("skill");
            let s = parse(&text, src, fallback);
            map.insert(s.name.clone(), s);
        }
    }

    map.into_values().collect()
}

pub fn get(name: &str) -> Option<Skill> {
    let want = name.trim().to_ascii_lowercase();
    all().into_iter().find(|s| s.name.to_ascii_lowercase() == want)
}

/// One line per skill for the system prompt.
pub fn index() -> String {
    all()
        .iter()
        .map(|s| {
            let tags = if s.tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", s.tags.join(","))
            };
            format!("- {} — {}{}", s.name, s.description, tags)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Full bodies for `--skill`, concatenated.
pub fn preload(names: &[String]) -> anyhow::Result<String> {
    let mut out = String::new();
    for n in names {
        let s = get(n).ok_or_else(|| {
            anyhow::anyhow!(
                "unknown skill '{}' — try one of: {}",
                n,
                all().iter().map(|s| s.name.clone()).collect::<Vec<_>>().join(", ")
            )
        })?;
        out.push_str(&format!("\n### skill: {} ({})\n{}\n", s.name, s.source, s.body));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_are_present_and_unique() {
        let all = all();
        assert!(all.len() >= 15, "expected the full pack, got {}", all.len());
        let mut names: Vec<&str> = all.iter().map(|s| s.name.as_str()).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), all.len(), "duplicate skill names");
        assert!(all.iter().all(|s| !s.description.is_empty()));
        assert!(all.iter().all(|s| s.body.len() > 200));
    }

    #[test]
    fn frontmatter_parses() {
        let text = "---\nname: test-skill\ndescription: does a thing\ntags: [a, b]\n---\n# Body\nhello";
        let s = parse(text, "builtin", "fallback");
        assert_eq!(s.name, "test-skill");
        assert_eq!(s.description, "does a thing");
        assert_eq!(s.tags, vec!["a", "b"]);
        assert!(s.body.starts_with("# Body"));
    }

    #[test]
    fn lookup_is_case_insensitive() {
        assert!(get("RECON").is_some());
        assert!(get("nope-nope").is_none());
    }
}
