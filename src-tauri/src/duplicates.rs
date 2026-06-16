//! 重复检测：DOI 优先 → 标题归一化 → 作者+年份。

use crate::error::AppResult;
use crate::types::DuplicateCandidate;
use rusqlite::params;
use std::path::Path;

pub fn normalize_doi(doi: &str) -> String {
    let s = doi.trim().to_lowercase();
    let s = s.strip_prefix("https://doi.org/").unwrap_or(&s);
    let s = s.strip_prefix("http://doi.org/").unwrap_or(s);
    let s = s.strip_prefix("doi:").unwrap_or(s);
    s.trim().to_string()
}

pub fn normalize_title(t: &str) -> String {
    let lower = t.to_lowercase();
    let cleaned: String = lower
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    let mut out = String::with_capacity(cleaned.len());
    let mut last_space = true;
    for ch in cleaned.chars() {
        if ch == ' ' {
            if !last_space {
                out.push(' ');
            }
            last_space = true;
        } else {
            out.push(ch);
            last_space = false;
        }
    }
    out.trim().to_string()
}

fn first_author_lastname(authors: &[String]) -> String {
    authors
        .first()
        .map(|a| {
            a.rsplit(' ')
                .next()
                .unwrap_or(a)
                .to_lowercase()
        })
        .unwrap_or_default()
}

/// 检测传入的元数据是否在库中已存在疑似重复。
pub fn detect(
    vault: &Path,
    doi: Option<&str>,
    title: Option<&str>,
    authors: Option<&[String]>,
    year: Option<i32>,
) -> AppResult<Vec<DuplicateCandidate>> {
    let conn = crate::db::open(vault)?;
    let mut candidates = Vec::new();

    if let Some(d) = doi {
        let nd = normalize_doi(d);
        if !nd.is_empty() {
            let mut stmt = conn.prepare("SELECT id, title FROM papers WHERE doi = ?1")?;
            let rows = stmt.query_map(params![nd], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?;
            for row in rows {
                let (id, t) = row?;
                candidates.push(DuplicateCandidate {
                    paper_id: id,
                    title: t,
                    reason: format!("DOI 相同 ({nd})"),
                    confidence: "high".into(),
                });
            }
        }
    }

    if candidates.is_empty() {
        if let Some(t) = title {
            let nt = normalize_title(t);
            if nt.len() >= 8 {
                let nt_prefix: String = nt.chars().take(100).collect();
                let mut stmt = conn.prepare(
                    "SELECT id, title, year FROM papers WHERE title IS NOT NULL",
                )?;
                let rows = stmt.query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, Option<i32>>(2)?,
                    ))
                })?;
                for row in rows {
                    let (id, db_title, db_year) = row?;
                    let db_nt = normalize_title(&db_title);
                    let db_prefix: String = db_nt.chars().take(100).collect();
                    if db_prefix == nt_prefix {
                        let year_match = match (year, db_year) {
                            (Some(a), Some(b)) => (a - b).abs() <= 1,
                            _ => true,
                        };
                        if year_match {
                            candidates.push(DuplicateCandidate {
                                paper_id: id,
                                title: db_title,
                                reason: "标题归一化匹配".into(),
                                confidence: "medium".into(),
                            });
                        }
                    }
                }
            }
        }
    }

    if candidates.is_empty() {
        if let (Some(authors), Some(year)) = (authors, year) {
            let last = first_author_lastname(authors);
            if !last.is_empty() {
                // P0：作者改从 paper_creators + creators 读取，避开已逻辑
                // 废弃的 papers.authors JSON 列。
                let mut stmt = conn.prepare(
                    "SELECT DISTINCT p.id, p.title, p.year,
                            (SELECT c.display_name
                             FROM paper_creators pc
                             JOIN creators c ON c.id = pc.creator_id
                             WHERE pc.paper_id = p.id
                             ORDER BY pc.position LIMIT 1) AS first_author
                     FROM papers p
                     WHERE p.year = ?1 OR p.year = ?2",
                )?;
                let rows = stmt.query_map(params![year, year + 1], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, Option<i32>>(2)?,
                        r.get::<_, Option<String>>(3)?,
                    ))
                })?;
                for row in rows {
                    let (id, db_title, db_year, db_first) = row?;
                    let last_db = db_first
                        .as_deref()
                        .map(|a| {
                            a.rsplit(' ').next().unwrap_or(a).to_lowercase()
                        })
                        .unwrap_or_default();
                    if last_db == last && db_year == Some(year) {
                        candidates.push(DuplicateCandidate {
                            paper_id: id,
                            title: db_title,
                            reason: format!("首作者+年份相同 ({last} {year})"),
                            confidence: "low".into(),
                        });
                    }
                }
            }
        }
    }

    Ok(candidates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn doi_normalize() {
        assert_eq!(normalize_doi("https://doi.org/10.1109/foo"), "10.1109/foo");
        assert_eq!(normalize_doi("DOI: 10.1109/FOO"), "10.1109/foo");
        assert_eq!(normalize_doi("  10.1109/bar  "), "10.1109/bar");
    }

    #[test]
    fn title_normalize() {
        assert_eq!(normalize_title("Hello,  World!! 2024"), "hello world 2024");
        assert_eq!(normalize_title("A   B"), "a b");
    }

    #[test]
    fn detect_by_doi() {
        let dir = tempdir().unwrap();
        crate::vault::init_at(dir.path()).unwrap();
        crate::db::migrate(dir.path()).unwrap();
        let conn = crate::db::open(dir.path()).unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        // P0: papers.status 受 CHECK 触发器约束，必须使用新枚举值。
        conn.execute(
            "INSERT INTO papers (id, title, doi, year, status, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params!["p1", "Existing", "10.1109/foo", 2024, "unread", now, now],
        ).unwrap();
        let dups = detect(dir.path(), Some("10.1109/foo"), None, None, None).unwrap();
        assert_eq!(dups.len(), 1);
        assert_eq!(dups[0].confidence, "high");
    }
}
