//! RIS 导出: Paper → CSL-JSON → RIS

use crate::export::csl;
use crate::types::Paper;

/// 渲染多篇 Paper 为 RIS 格式字符串。
/// 每篇论文经 CSL-JSON 中间表示后转为 RIS，以 ER 标记分隔。
pub fn render(papers: &[Paper]) -> String {
    let mut s = String::new();
    for p in papers {
        let csl = csl::paper_to_csl(p);
        s.push_str(&csl::csl_to_ris(&csl));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_paper() -> Paper {
        Paper {
            id: "p1".into(),
            title: "Hello, World!".into(),
            authors: vec!["Alice Smith".into(), "Bob Doe".into()],
            year: Some(2024),
            venue: "Proc. Conf. ML".into(),
            doi: "10.1109/foo".into(),
            abstract_text: "An abstract.".into(),
            keywords: vec!["ml".into(), "deep".into()],
            ..Default::default()
        }
    }

    #[test]
    fn test_ris_render_basic() {
        let s = render(&[sample_paper()]);
        // 完整 RIS 输出格式
        assert!(s.contains("TY  - CONF"));
        assert!(s.contains("TI  - Hello, World!"));
        assert!(s.contains("AU  - Smith, Alice"));
        assert!(s.contains("AU  - Doe, Bob"));
        assert!(s.contains("PY  - 2024"));
        assert!(s.contains("T2  - Proc. Conf. ML"));
        assert!(s.contains("DO  - 10.1109/foo"));
        assert!(s.contains("AB  - An abstract."));
        assert!(s.contains("KW  - ml"));
        assert!(s.contains("KW  - deep"));
        // 以 ER 结束
        assert!(s.contains("ER  - "));
        // 只有一篇 → 一个 TY 和一个 ER
        assert_eq!(s.matches("TY  - ").count(), 1);
        assert_eq!(s.matches("ER  - ").count(), 1);
    }

    #[test]
    fn test_ris_render_multiple() {
        let papers = [
            sample_paper(),
            Paper {
                id: "p2".into(),
                title: "Second Paper".into(),
                authors: vec!["Carol Doe".into()],
                year: Some(2023),
                venue: "Nature".into(),
                doi: "10.1038/bar".into(),
                ..Default::default()
            },
        ];
        let s = render(&papers);
        // 两篇 → 两个 TY 和两个 ER
        assert_eq!(s.matches("TY  - ").count(), 2);
        assert_eq!(s.matches("ER  - ").count(), 2);
        // 第一篇
        assert!(s.contains("TI  - Hello, World!"));
        // 第二篇 (article-journal → JOUR)
        assert!(s.contains("TY  - JOUR"));
        assert!(s.contains("TI  - Second Paper"));
        assert!(s.contains("AU  - Doe, Carol"));
        assert!(s.contains("PY  - 2023"));
    }
}
