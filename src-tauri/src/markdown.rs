//! Markdown 笔记：frontmatter 解析、模板、AI 区块安全替换。

use crate::error::{AppError, AppResult};
use std::path::Path;

const FRONTMATTER_DELIM: &str = "---";
const AI_BLOCK_START: &str = "<!-- AI_GENERATED_START:";
const AI_BLOCK_END: &str = "<!-- AI_GENERATED_END:";

#[derive(Debug, Default, Clone)]
pub struct NoteContent {
    pub frontmatter: serde_yaml::Value,
    pub body: String,
    pub raw: String,
}

/// 读 markdown 文件，分离 frontmatter 和 body。
/// 没有 frontmatter 时 frontmatter 为 Null。
pub fn read_note(path: &Path) -> AppResult<NoteContent> {
    let raw = std::fs::read_to_string(path)?;
    if let Some(rest) = raw.strip_prefix(FRONTMATTER_DELIM) {
        if let Some(idx) = rest.find(FRONTMATTER_DELIM) {
            let yaml_str = &rest[..idx];
            let body = rest[idx + FRONTMATTER_DELIM.len()..]
                .trim_start_matches('\n')
                .to_string();
            let fm: serde_yaml::Value = serde_yaml::from_str(yaml_str).unwrap_or(serde_yaml::Value::Null);
            return Ok(NoteContent { frontmatter: fm, body, raw });
        }
    }
    Ok(NoteContent {
        frontmatter: serde_yaml::Value::Null,
        body: raw.clone(),
        raw,
    })
}

/// 写回。frontmatter + body 重新拼接。
pub fn write_note(path: &Path, fm: &serde_yaml::Value, body: &str) -> AppResult<()> {
    let yaml = serde_yaml::to_string(fm).unwrap_or_else(|_| "".into());
    let mut s = String::new();
    s.push_str(FRONTMATTER_DELIM);
    s.push('\n');
    s.push_str(&yaml);
    s.push_str(FRONTMATTER_DELIM);
    s.push('\n');
    s.push_str(body);
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    std::fs::write(path, s)?;
    Ok(())
}

/// 默认论文笔记模板。
pub fn default_template(meta: &crate::types::Paper) -> String {
    let title = if meta.title.is_empty() {
        "（待补全标题）".to_string()
    } else {
        meta.title.clone()
    };
    format!(
        r#"# {title}

## 基本信息

- 作者：{authors}
- 年份：{year}
- 期刊/会议：{venue}
- DOI：{doi}
- 关键词：{keywords}
- 标签：{tags}

## AI 摘要
{AI_START_SUMMARY}
{AI_END_SUMMARY}

## 论文要点
{AI_START_KEY}
{AI_END_KEY}

## 方法理解

## 实验与结果

## 局限与问题

## 我的笔记

## 相关论文
"#,
        title = title,
        authors = meta.authors.join(", "),
        year = meta.year.map(|y| y.to_string()).unwrap_or_default(),
        venue = meta.venue,
        doi = meta.doi,
        keywords = meta.keywords.join(", "),
        tags = meta.tags.join(", "),
        AI_START_SUMMARY = AI_BLOCK_START.replace("START", "START:summary"),
        AI_END_SUMMARY = AI_BLOCK_END.replace("END", "END:summary"),
        AI_START_KEY = AI_BLOCK_START.replace("START", "START:key_points"),
        AI_END_KEY = AI_BLOCK_END.replace("END", "END:key_points"),
    )
}

/// AI 区块名常量
pub const BLOCK_SUMMARY: &str = "summary";
pub const BLOCK_KEY_POINTS: &str = "key_points";

/// 安全更新 AI 区块。如果原笔记没有该区块则不动。
pub fn update_ai_block(path: &Path, block: &str, new_content: &str) -> AppResult<()> {
    let nc = read_note(path)?;
    let start_marker = format!("{AI_BLOCK_START}{block} -->");
    let end_marker = format!("{AI_BLOCK_END}{block} -->");

    let body = nc.body;
    let new_body = replace_ai_section(&body, &start_marker, &end_marker, new_content)
        .ok_or_else(|| AppError::Markdown(format!("笔记中找不到 AI 区块 {block}")))?;

    write_note(path, &nc.frontmatter, &new_body)?;
    Ok(())
}

fn replace_ai_section(body: &str, start: &str, end: &str, new: &str) -> Option<String> {
    let s = body.find(start)?;
    let after_start = s + start.len();
    let e = body[after_start..].find(end)?;
    let e_abs = after_start + e;
    let mut out = String::with_capacity(body.len() + new.len());
    out.push_str(&body[..after_start]);
    out.push('\n');
    out.push_str(new);
    if !new.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&body[e_abs..]);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn read_write_roundtrip() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("note.md");
        let fm: serde_yaml::Value = serde_yaml::from_str("title: hello\nyear: 2024").unwrap();
        write_note(&p, &fm, "# body\ntext").unwrap();
        let nc = read_note(&p).unwrap();
        assert_eq!(nc.body, "# body\ntext");
        assert_eq!(
            nc.frontmatter.get("title").and_then(|v| v.as_str()),
            Some("hello")
        );
    }

    #[test]
    fn ai_block_update_preserves_outside() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("note.md");
        let body = r#"# Title

## AI 摘要
<!-- AI_GENERATED_START:summary -->
old summary
<!-- AI_GENERATED_END:summary -->

## 我的笔记
my hand-written
"#;
        std::fs::write(&p, body).unwrap();
        update_ai_block(&p, BLOCK_SUMMARY, "new summary\n\nline2").unwrap();
        let nc = read_note(&p).unwrap();
        assert!(nc.body.contains("new summary"));
        assert!(nc.body.contains("line2"));
        assert!(nc.body.contains("my hand-written"));
        assert!(!nc.body.contains("old summary"));
    }

    #[test]
    fn ai_block_missing_returns_err() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("note.md");
        std::fs::write(&p, "# No block").unwrap();
        assert!(update_ai_block(&p, BLOCK_SUMMARY, "x").is_err());
    }
}
