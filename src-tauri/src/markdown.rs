use crate::project::Result;
use regex::Regex;
use std::sync::LazyLock;
use yaml_edit::{Document as YamlDocument, ScalarValue};

static CHAPTER_PREFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:chapter[ \t]+[0-9]+|第[0-9零〇一二三四五六七八九十百千两]+[章回节])(?:[ \t]*[:：.、—–-][ \t]*|[ \t]+)")
        .unwrap()
});

pub(crate) struct Markdown<'a> {
    pub body: &'a str,
    pub title: Option<String>,
    pub needs_title_migration: bool,
    yaml: Option<&'a str>,
}

impl<'a> Markdown<'a> {
    pub fn read(source: &'a str, chapter: bool, fallback: &str) -> Self {
        let (yaml, body, metadata) = frontmatter(source)
            .and_then(|(yaml, body)| {
                let metadata = yaml.parse::<YamlDocument>().ok();
                (empty_yaml(yaml)
                    || metadata
                        .as_ref()
                        .is_some_and(|doc| doc.as_mapping().is_some()))
                .then(|| (Some(yaml), without_blank_separator(body), metadata))
            })
            .unwrap_or((None, source, None));
        let title = metadata
            .and_then(|metadata| metadata.get_string("title"))
            .map(|value| value.replace(['\r', '\n'], " ").trim().to_owned())
            .filter(|value| !value.is_empty());
        if !chapter {
            return Self {
                body,
                title,
                needs_title_migration: false,
                yaml,
            };
        }

        let heading = leading_heading(body).filter(|(heading, _)| {
            title
                .as_deref()
                .is_none_or(|title| title == without_chapter_prefix(heading))
        });
        let needs_title_migration = title.is_none() || heading.is_some();
        let title = title.or_else(|| {
            Some(heading.map_or_else(
                || {
                    fallback
                        .split_once('-')
                        .filter(|(prefix, suffix)| {
                            !prefix.is_empty()
                                && prefix.bytes().all(|byte| byte.is_ascii_digit())
                                && !suffix.is_empty()
                        })
                        .map_or(fallback, |(_, suffix)| suffix)
                        .to_owned()
                },
                |(heading, _)| without_chapter_prefix(heading).to_owned(),
            ))
        });
        Self {
            body: heading.map_or(body, |(_, rest)| rest),
            title,
            needs_title_migration,
            yaml,
        }
    }

    pub fn save(&self, source: &str, body: &str, title: Option<&str>) -> Result<String> {
        if let Some(title) = title {
            let title = title.trim();
            if title.is_empty() || title.chars().any(char::is_control) {
                return Err("请输入单行、非空的文档标题。".into());
            }
            if self.needs_title_migration || self.title.as_deref() != Some(title) {
                let mut yaml = set_title(self.yaml.unwrap_or_default(), title)?;
                if !yaml.ends_with('\n') {
                    yaml.push('\n');
                }
                return Ok(format!("---\n{yaml}---\n\n{body}"));
            }
        }

        let prefix = &source[..source.len() - self.body.len()];
        let separator = if !prefix.is_empty() && !prefix.ends_with('\n') && !body.is_empty() {
            "\n\n"
        } else {
            ""
        };
        Ok(format!("{prefix}{separator}{body}"))
    }
}

fn frontmatter(text: &str) -> Option<(&str, &str)> {
    let (opening, remaining) = text.split_once('\n')?;
    if opening.trim_end() != "---" {
        return None;
    }

    let mut offset = 0;
    for line in remaining.split_inclusive('\n') {
        if matches!(line.trim_end(), "---" | "...") {
            return Some((&remaining[..offset], &remaining[offset + line.len()..]));
        }
        offset += line.len();
    }
    None
}

fn without_blank_separator(text: &str) -> &str {
    text.strip_prefix('\n').unwrap_or(text)
}

fn leading_heading(text: &str) -> Option<(&str, &str)> {
    let text = text.trim_start_matches('\n');
    let (line, body) = text.split_once('\n').unwrap_or((text, ""));
    let title = line.strip_prefix("# ")?.trim();
    (!title.is_empty()).then_some((title, without_blank_separator(body)))
}

fn without_chapter_prefix(title: &str) -> &str {
    CHAPTER_PREFIX.find(title).map_or(title, |matched| {
        let remainder = title[matched.end()..].trim_start();
        if remainder.is_empty() {
            title
        } else {
            remainder
        }
    })
}

fn set_title(yaml: &str, title: &str) -> Result<String> {
    if empty_yaml(yaml) {
        let title = serde_json::to_string(title).map_err(|error| error.to_string())?;
        return Ok(format!("{yaml}title: {title}\n"));
    }
    let metadata: YamlDocument = yaml
        .parse()
        .map_err(|error| format!("无法读取已有 frontmatter：{error}"))?;
    if !metadata.set("title", ScalarValue::string(title)) {
        return Err("已有 frontmatter 必须是 YAML 映射，才能修改标题。".into());
    }
    Ok(metadata.to_string())
}

fn empty_yaml(yaml: &str) -> bool {
    yaml.lines().all(|line| {
        let line = line.trim();
        line.is_empty() || line.starts_with('#')
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_title_without_rewriting_other_frontmatter_or_body() {
        let source =
            "---\n# description\ntitle: '旧标题' # title note\ntags: [a, b]\n---\n\n段落。\n";
        let parts = Markdown::read(source, true, "old");
        assert_eq!(parts.title.as_deref(), Some("旧标题"));
        assert_eq!(parts.body, "段落。\n");
        let saved = parts
            .save(source, parts.body, Some("新标题: \"引文\""))
            .unwrap();
        let updated = Markdown::read(&saved, true, "old");
        assert_eq!(updated.title.as_deref(), Some("新标题: \"引文\""));
        assert_eq!(updated.body, parts.body);
        assert!(saved.contains("# description\n"));
        assert!(saved.contains("# title note\n"));
        assert!(saved.contains("tags: [a, b]\n"));
    }

    #[test]
    fn retains_nonchapter_headings_and_rules() {
        let manual = "---\ntitle: Manual\n---\n\n# Introduction\n\n正文\n";
        let parts = Markdown::read(manual, false, "manual");
        assert_eq!(parts.title.as_deref(), Some("Manual"));
        assert_eq!(parts.body, "# Introduction\n\n正文\n");
        let document = "---\n\nPlain prose\n\n---\n";
        assert_eq!(Markdown::read(document, false, "note").body, document);
    }
}
