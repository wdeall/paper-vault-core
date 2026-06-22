//! BibTeX 导出。

use crate::types::Paper;

/// 由 metadata 推断 entry type。
/// 有 journal → article；booktitle → inproceedings；其他 → misc。
#[allow(dead_code)]
pub fn entry_type(p: &Paper) -> &'static str {
    if !p.venue.is_empty() {
        // 粗略：包含 proceedings 字样当 inproceedings
        let v = p.venue.to_lowercase();
        if v.contains("conf") || v.contains("proceeding") || v.contains("workshop") {
            return "inproceedings";
        }
        return "article";
    }
    "misc"
}

/// 用 title 转 cite key：首词小写 + 年份 + 首作者姓氏首字母。
#[allow(dead_code)]
pub fn cite_key(p: &Paper) -> String {
    let first = p
        .authors
        .first()
        .map(|a| {
            a.trim()
                .chars()
                .next()
                .unwrap_or('x')
                .to_ascii_lowercase()
        })
        .unwrap_or('x');
    let word = p
        .title
        .split_whitespace()
        .next()
        .unwrap_or("untitled")
        .to_lowercase();
    let word: String = word
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    let word = if word.is_empty() { "untitled".to_string() } else { word };
    let year = p.year.map(|y| y.to_string()).unwrap_or_default();
    format!("{first}{word}{year}")
}

pub fn escape_bibtex(s: &str) -> String {
    // 简化转义：把 \ { } $ & % # _ ^ 替换为 LaTeX 转义
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str(r"\textbackslash{}"),
            '{' => out.push_str(r"\{"),
            '}' => out.push_str(r"\}"),
            '$' => out.push_str(r"\$"),
            '&' => out.push_str(r"\&"),
            '%' => out.push_str(r"\%"),
            '#' => out.push_str(r"\#"),
            '_' => out.push_str(r"\_"),
            '^' => out.push_str(r"\^{}"),
            '~' => out.push_str(r"\~{}"),
            '\n' => out.push_str(" "),
            c => out.push(c),
        }
    }
    out
}

pub fn render(papers: &[Paper]) -> String {
    // SPEC §3.6: Paper → CSL-JSON → BibTeX
    let mut s = String::new();
    for p in papers {
        let csl = crate::export::csl::paper_to_csl(p);
        s.push_str(&crate::export::csl::csl_to_bibtex(&csl));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p() -> Paper {
        Paper {
            title: "Hello, World!".into(),
            authors: vec!["Alice Smith".into(), "Bob".into()],
            year: Some(2024),
            venue: "Proc. Conf. ML".into(),
            doi: "10.1109/foo".into(),
            ..Default::default()
        }
    }

    #[test]
    fn entry_type_decision() {
        assert_eq!(entry_type(&p()), "inproceedings");
        let mut q = p();
        q.venue = "Nature".into();
        assert_eq!(entry_type(&q), "article");
        let mut r = p();
        r.venue = String::new();
        assert_eq!(entry_type(&r), "misc");
    }

    #[test]
    fn cite_key_format() {
        let k = cite_key(&p());
        assert!(k.starts_with('a'));
        assert!(k.contains("hello"));
        assert!(k.ends_with("2024"));
    }

    #[test]
    fn render_contains_key_fields() {
        let s = render(&[p()]);
        assert!(s.contains("@inproceedings"));
        // CSL 路径: author 为 "family, given" 格式
        assert!(s.contains("Smith, Alice and Bob"));
        assert!(s.contains("booktitle"));
        assert!(s.contains("Hello, World!"));
    }

    #[test]
    fn escape_handles_specials() {
        let esc = escape_bibtex("a_b & c#d");
        assert!(esc.contains(r"\_"));
        assert!(esc.contains(r"\&"));
        assert!(esc.contains(r"\#"));
    }
}
