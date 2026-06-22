//! CSL-JSON 中间表示: Paper → CSL-JSON → BibTeX / RIS
//!
//! SPEC §3.6: 转换路径 Paper (DB) → CSL-JSON (统一表示) → BibTeX / RIS。
//! 不引入 citeproc-rs，仅做结构化字段映射。

use crate::export::bibtex::escape_bibtex;
use crate::types::Paper;
use serde_json::{json, Value};

/// 由 venue 推断 CSL type。
/// - venue 含 conf/proceeding/workshop → "paper-conference"
/// - venue 非空 → "article-journal"
/// - venue 空 → "manuscript"
fn csl_type(p: &Paper) -> &'static str {
    if p.venue.is_empty() {
        return "manuscript";
    }
    let v = p.venue.to_lowercase();
    if v.contains("conf") || v.contains("proceeding") || v.contains("workshop") {
        return "paper-conference";
    }
    "article-journal"
}

/// 解析单个作者名为 (family, given)。
/// "Alice Smith" → ("Smith", "Alice")；无空格 → (全名, "")。
fn split_author(name: &str) -> (String, String) {
    let name = name.trim();
    if let Some(idx) = name.rfind(' ') {
        let given = name[..idx].trim().to_string();
        let family = name[idx + 1..].trim().to_string();
        (family, given)
    } else {
        (name.to_string(), String::new())
    }
}

/// Paper → CSL-JSON (单个论文)。
pub fn paper_to_csl(paper: &Paper) -> Value {
    let authors: Vec<Value> = paper
        .authors
        .iter()
        .map(|a| {
            let (family, given) = split_author(a);
            json!({"family": family, "given": given})
        })
        .collect();

    let mut csl = json!({
        "id": paper.id,
        "type": csl_type(paper),
        "title": paper.title,
    });

    if !authors.is_empty() {
        csl["author"] = Value::Array(authors);
    }
    if let Some(y) = paper.year {
        csl["issued"] = json!({"date-parts": [[y]]});
    }
    if !paper.venue.is_empty() {
        csl["container-title"] = json!(paper.venue);
    }
    if !paper.doi.is_empty() {
        csl["DOI"] = json!(paper.doi);
    }
    if !paper.abstract_text.is_empty() {
        csl["abstract"] = json!(paper.abstract_text);
    }
    if !paper.keywords.is_empty() {
        csl["keyword"] = json!(paper.keywords.join(", "));
    }
    csl
}

/// 多篇 Paper → CSL-JSON 数组字符串 (pretty printed)。
pub fn papers_to_csl_json(papers: &[Paper]) -> String {
    let arr: Vec<Value> = papers.iter().map(paper_to_csl).collect();
    serde_json::to_string_pretty(&Value::Array(arr)).unwrap_or_else(|_| "[]".to_string())
}

/// 从 CSL-JSON 计算 BibTeX cite key:
/// 首作者姓氏首字母 + title 首词 (小写 alphanumeric) + year。
fn csl_cite_key(csl: &Value) -> String {
    let first = csl["author"][0]["family"]
        .as_str()
        .and_then(|s| s.chars().next())
        .unwrap_or('x')
        .to_ascii_lowercase();
    let title = csl["title"].as_str().unwrap_or("");
    let word = title
        .split_whitespace()
        .next()
        .unwrap_or("untitled")
        .to_lowercase();
    let word: String = word.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    let word = if word.is_empty() {
        "untitled".to_string()
    } else {
        word
    };
    let year = csl["issued"]["date-parts"][0][0]
        .as_i64()
        .map(|y| y.to_string())
        .unwrap_or_default();
    format!("{first}{word}{year}")
}

/// CSL type → BibTeX entry type。
fn csl_to_bibtex_type(t: &str) -> &'static str {
    match t {
        "article-journal" => "article",
        "paper-conference" => "inproceedings",
        _ => "misc",
    }
}

/// CSL-JSON → BibTeX 条目。
pub fn csl_to_bibtex(csl: &Value) -> String {
    let et = csl_to_bibtex_type(csl["type"].as_str().unwrap_or("manuscript"));
    let key = csl_cite_key(csl);
    let mut s = String::new();
    s.push_str(&format!("@{et}{{{key},\n"));

    if let Some(title) = csl["title"].as_str() {
        s.push_str(&format!("  title  = {{{}}},\n", escape_bibtex(title)));
    }

    // author: "family, given" 格式，用 " and " 连接
    if let Some(authors) = csl["author"].as_array() {
        let parts: Vec<String> = authors
            .iter()
            .map(|a| {
                let family = a["family"].as_str().unwrap_or("");
                let given = a["given"].as_str().unwrap_or("");
                if given.is_empty() {
                    family.to_string()
                } else {
                    format!("{family}, {given}")
                }
            })
            .collect();
        if !parts.is_empty() {
            s.push_str(&format!(
                "  author = {{{}}},\n",
                escape_bibtex(&parts.join(" and "))
            ));
        }
    }

    if let Some(y) = csl["issued"]["date-parts"][0][0].as_i64() {
        s.push_str(&format!("  year   = {{{y}}},\n"));
    }

    if let Some(ct) = csl["container-title"].as_str() {
        if !ct.is_empty() {
            let field = if et == "inproceedings" {
                "booktitle"
            } else {
                "journal"
            };
            s.push_str(&format!("  {field} = {{{}}},\n", escape_bibtex(ct)));
        }
    }

    if let Some(doi) = csl["DOI"].as_str() {
        if !doi.is_empty() {
            s.push_str(&format!("  doi    = {{{}}},\n", escape_bibtex(doi)));
        }
    }

    if let Some(abs) = csl["abstract"].as_str() {
        if !abs.is_empty() {
            s.push_str(&format!("  abstract = {{{}}},\n", escape_bibtex(abs)));
        }
    }

    if let Some(kw) = csl["keyword"].as_str() {
        if !kw.is_empty() {
            s.push_str(&format!("  keywords = {{{}}},\n", escape_bibtex(kw)));
        }
    }

    s.push_str("}\n\n");
    s
}

/// CSL type → RIS TY。
fn csl_to_ris_type(t: &str) -> &'static str {
    match t {
        "article-journal" => "JOUR",
        "paper-conference" => "CONF",
        "book" => "BOOK",
        _ => "GEN",
    }
}

/// CSL-JSON → RIS 条目。
pub fn csl_to_ris(csl: &Value) -> String {
    let mut s = String::new();
    let ty = csl_to_ris_type(csl["type"].as_str().unwrap_or("manuscript"));
    s.push_str(&format!("TY  - {ty}\n"));

    if let Some(title) = csl["title"].as_str() {
        s.push_str(&format!("TI  - {title}\n"));
    }

    if let Some(authors) = csl["author"].as_array() {
        for a in authors {
            let family = a["family"].as_str().unwrap_or("");
            let given = a["given"].as_str().unwrap_or("");
            if given.is_empty() {
                s.push_str(&format!("AU  - {family}\n"));
            } else {
                s.push_str(&format!("AU  - {family}, {given}\n"));
            }
        }
    }

    if let Some(y) = csl["issued"]["date-parts"][0][0].as_i64() {
        s.push_str(&format!("PY  - {y}\n"));
    }

    if let Some(ct) = csl["container-title"].as_str() {
        if !ct.is_empty() {
            s.push_str(&format!("T2  - {ct}\n"));
        }
    }

    if let Some(doi) = csl["DOI"].as_str() {
        if !doi.is_empty() {
            s.push_str(&format!("DO  - {doi}\n"));
        }
    }

    if let Some(abs) = csl["abstract"].as_str() {
        if !abs.is_empty() {
            s.push_str(&format!("AB  - {abs}\n"));
        }
    }

    // KW: 每个关键词一行
    if let Some(kw) = csl["keyword"].as_str() {
        if !kw.is_empty() {
            for k in kw.split(',') {
                let k = k.trim();
                if !k.is_empty() {
                    s.push_str(&format!("KW  - {k}\n"));
                }
            }
        }
    }

    s.push_str("ER  - \n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_paper() -> Paper {
        Paper {
            id: "p1".into(),
            title: "Hello, World!".into(),
            authors: vec!["Alice Smith".into(), "Bob".into()],
            year: Some(2024),
            venue: "Proc. Conf. ML".into(),
            doi: "10.1109/foo".into(),
            abstract_text: "An abstract.".into(),
            keywords: vec!["ml".into(), "deep".into()],
            ..Default::default()
        }
    }

    #[test]
    fn test_paper_to_csl_basic() {
        let csl = paper_to_csl(&sample_paper());
        assert_eq!(csl["id"].as_str(), Some("p1"));
        assert_eq!(csl["type"].as_str(), Some("paper-conference"));
        assert_eq!(csl["title"].as_str(), Some("Hello, World!"));
        assert_eq!(csl["DOI"].as_str(), Some("10.1109/foo"));
        assert_eq!(csl["issued"]["date-parts"][0][0].as_i64(), Some(2024));
        assert_eq!(csl["container-title"].as_str(), Some("Proc. Conf. ML"));
        assert_eq!(csl["abstract"].as_str(), Some("An abstract."));
    }

    #[test]
    fn test_paper_to_csl_author_parsing() {
        let csl = paper_to_csl(&sample_paper());
        let authors = csl["author"].as_array().unwrap();
        assert_eq!(authors.len(), 2);
        assert_eq!(authors[0]["family"].as_str(), Some("Smith"));
        assert_eq!(authors[0]["given"].as_str(), Some("Alice"));
        // 无空格 → family=全名, given=""
        assert_eq!(authors[1]["family"].as_str(), Some("Bob"));
        assert_eq!(authors[1]["given"].as_str(), Some(""));
    }

    #[test]
    fn test_paper_to_csl_type_inference() {
        // venue 含 "conf" → paper-conference
        let mut p = sample_paper();
        p.venue = "Conf. NIPS".into();
        assert_eq!(paper_to_csl(&p)["type"].as_str(), Some("paper-conference"));

        // venue 含 "proceeding"
        p.venue = "Proceedings of ICML".into();
        assert_eq!(paper_to_csl(&p)["type"].as_str(), Some("paper-conference"));

        // venue 含 "workshop"
        p.venue = "Workshop on X".into();
        assert_eq!(paper_to_csl(&p)["type"].as_str(), Some("paper-conference"));

        // venue 非空普通 → article-journal
        p.venue = "Nature".into();
        assert_eq!(paper_to_csl(&p)["type"].as_str(), Some("article-journal"));

        // venue 空 → manuscript
        p.venue = String::new();
        assert_eq!(paper_to_csl(&p)["type"].as_str(), Some("manuscript"));
    }

    #[test]
    fn test_csl_to_bibtex() {
        let csl = paper_to_csl(&sample_paper());
        let bib = csl_to_bibtex(&csl);
        // paper-conference → inproceedings
        assert!(bib.contains("@inproceedings{"));
        // author: "family, given" 格式
        assert!(bib.contains("Smith, Alice and Bob"));
        assert!(bib.contains("title  = {Hello, World!}"));
        assert!(bib.contains("year   = {2024}"));
        assert!(bib.contains("booktitle = {Proc. Conf. ML}"));
        assert!(bib.contains("doi    = {10.1109/foo}"));
        // cite key: 首作者姓氏首字母 s + hello + 2024
        assert!(bib.contains("shello2024"));

        // article-journal → article
        let mut p = sample_paper();
        p.venue = "Nature".into();
        let bib2 = csl_to_bibtex(&paper_to_csl(&p));
        assert!(bib2.contains("@article{"));
        assert!(bib2.contains("journal = {Nature}"));
    }

    #[test]
    fn test_csl_to_ris() {
        let csl = paper_to_csl(&sample_paper());
        let ris = csl_to_ris(&csl);
        // paper-conference → CONF
        assert!(ris.contains("TY  - CONF"));
        assert!(ris.contains("TI  - Hello, World!"));
        assert!(ris.contains("AU  - Smith, Alice"));
        assert!(ris.contains("AU  - Bob"));
        assert!(ris.contains("PY  - 2024"));
        assert!(ris.contains("T2  - Proc. Conf. ML"));
        assert!(ris.contains("DO  - 10.1109/foo"));
        assert!(ris.contains("AB  - An abstract."));
        assert!(ris.contains("ER  - "));
    }

    #[test]
    fn test_csl_to_ris_multiple_keywords() {
        let csl = paper_to_csl(&sample_paper());
        let ris = csl_to_ris(&csl);
        // 两个关键词 → 两行 KW
        assert!(ris.contains("KW  - ml"));
        assert!(ris.contains("KW  - deep"));
        let kw_count = ris.matches("KW  - ").count();
        assert_eq!(kw_count, 2);
    }

    #[test]
    fn test_papers_to_csl_json_array() {
        let papers = [
            sample_paper(),
            Paper {
                id: "p2".into(),
                title: "Second".into(),
                authors: vec!["Carol Doe".into()],
                year: Some(2023),
                venue: "Science".into(),
                ..Default::default()
            },
        ];
        let json_str = papers_to_csl_json(&papers);
        // 应为 JSON 数组
        assert!(json_str.trim_start().starts_with('['));
        assert!(json_str.trim_end().ends_with(']'));
        // 解析回来验证
        let parsed: Vec<Value> = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0]["id"].as_str(), Some("p1"));
        assert_eq!(parsed[1]["id"].as_str(), Some("p2"));
        assert_eq!(parsed[1]["type"].as_str(), Some("article-journal"));
    }
}
