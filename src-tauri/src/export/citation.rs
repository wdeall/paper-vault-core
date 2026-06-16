//! Markdown 引用导出：`- {firstAuthor}, {year}, {title}, DOI:{doi}`

use crate::types::Paper;

pub fn render(papers: &[Paper]) -> String {
    let mut out = String::new();
    for p in papers {
        let author = p
            .authors
            .first()
            .cloned()
            .unwrap_or_else(|| "(unknown)".into());
        let year = p
            .year
            .map(|y| y.to_string())
            .unwrap_or_else(|| "n.d.".into());
        let title = if p.title.is_empty() { "(untitled)" } else { &p.title };
        let doi = if p.doi.is_empty() { "—".into() } else { p.doi.clone() };
        out.push_str(&format!("- {author}, {year}, {title}, DOI:{doi}\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn basic_format() {
        let p = Paper {
            title: "X".into(),
            authors: vec!["A".into()],
            year: Some(2024),
            doi: "10.1/x".into(),
            ..Default::default()
        };
        let s = render(&[p]);
        assert!(s.contains("A, 2024, X, DOI:10.1/x"));
    }
}
