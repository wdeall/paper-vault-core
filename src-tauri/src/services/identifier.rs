//! Identifier 解析：DOI / arXiv / PMID / ISBN。
//!
//! 输入支持 4 种形态：
//!   1. 纯标识符（"10.1109/foo" / "2401.01234" / "12345678" / "9787123456789"）
//!   2. URL（"https://doi.org/10.xxx" / "https://arxiv.org/abs/..." /
//!         "https://pubmed.ncbi.nlm.nih.gov/12345"）
//!   3. 带前缀（"DOI: 10.xxx" / "arXiv: 2401.01234"）
//!   4. 任意字符串（尽量从中抽出一个有效 identifier）
//!
//! 设计原则：返回 `Vec` 而非 `Option` —— 一个长串里可能既包含
//! arXiv 又包含 DOI（例如某些会议模板同时打印两个）。调用方按
//! 顺序消费。

use once_cell::sync::Lazy;
use regex::Regex;

/// 支持的 identifier scheme 枚举。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scheme {
    Doi,
    Arxiv,
    Pmid,
    Isbn,
}

impl Scheme {
    pub fn as_str(self) -> &'static str {
        match self {
            Scheme::Doi => "doi",
            Scheme::Arxiv => "arxiv",
            Scheme::Pmid => "pmid",
            Scheme::Isbn => "isbn",
        }
    }
}

impl std::fmt::Display for Scheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for Scheme {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "doi" => Ok(Scheme::Doi),
            "arxiv" | "arXiv" => Ok(Scheme::Arxiv),
            "pmid" => Ok(Scheme::Pmid),
            "isbn" => Ok(Scheme::Isbn),
            other => Err(format!("unknown scheme: {other}")),
        }
    }
}

// -----------------------------------------------------------
// 正则
// -----------------------------------------------------------

// DOI: 严格 10.NNNN/... 形式（至少 4 位数字）
static RE_DOI: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"10\.\d{4,9}/[-._;()/:A-Za-z0-9]+").unwrap()
});

// DOI 在 URL 上下文里出现：先取 URL 路径再去 RE_DOI。
//   https://doi.org/10.1109/foo
//   https://dx.doi.org/10.1109/foo
//   doi.org/10.1109/foo
static RE_DOI_URL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)https?://(?:dx\.)?doi\.org/(10\.\d{4,9}/[-._;()/:A-Za-z0-9]+)")
        .unwrap()
});

// arXiv URL：https://arxiv.org/abs/2401.01234  /  /pdf/2401.01234
static RE_ARXIV_URL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)arxiv\.org/(?:abs|pdf)/([0-9]{4}\.[0-9]{4,5}(?:v[0-9]+)?)")
        .unwrap()
});

// arXiv 旧格式 URL：cs.LG/0703001
static RE_ARXIV_OLD_URL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)arxiv\.org/(?:abs|pdf)/([a-z\-]+(?:\.[A-Z]{2})?/\d{7}(?:v\d+)?)")
        .unwrap()
});

// arXiv 新格式（裸）：2401.01234 或 2401.01234v2
// 不使用 look-behind（regex 1.x 不支持），改为 find + 上下文判断：
// 命中位置的前后字符不能是 [A-Za-z0-9.] —— 这样可以避免
// "CVPR.2020.01234" 中的 "2020.01234" 被误匹配。
static RE_ARXIV_NEW: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(\d{4}\.\d{4,5}(?:v\d+)?)").unwrap()
});

// arXiv 旧格式（裸）：cs.LG/0703001
static RE_ARXIV_OLD: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"([a-z\-]+(?:\.[A-Z]{2})?/\d{7}(?:v\d+)?)").unwrap()
});

// PMID URL：https://pubmed.ncbi.nlm.nih.gov/12345
static RE_PMID_URL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"pubmed\.ncbi\.nlm\.nih\.gov/(\d{1,8})").unwrap()
});

// 纯 PMID（带前缀）：PMID: 12345  /  pmid 12345
static RE_PMID_BARE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)pmid[:\s]+(\d{1,8})\b").unwrap()
});

// 纯 PMID（裸数字）：仅当整段输入是一个独立数字时使用——
// 由调用方在 parse() 末尾根据 trim 结果单独判断，避免把
// "Published in 2024" 中的 "2024" 误判为 PMID。

// ISBN 形态：10 位 / 13 位数字（含可能的 - 分隔）。从一段字符串里
// 抽 ISBN-13 较危险（与 EAN-13 重叠）；仅匹配 10/13 位连续数字或
// 带短横线（X 结尾允许）。
static RE_ISBN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b(?:\d{9}[\dXx]|\d{13}|\d{1,5}-\d{1,7}-\d{1,7}-[\dXx]|\d{1,5}-\d{1,7}-\d{1,7}-\d{1,7}-\d)\b")
        .unwrap()
});

/// 解析输入字符串，返回一组 `(scheme, value)`。
///
/// 顺序：DOI → arXiv → PMID → ISBN（DOI 优先级最高；PMID 数字匹配放在最后以免和年份混淆）。
pub fn parse(input: &str) -> Vec<(Scheme, String)> {
    let s = input.trim();
    if s.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<(Scheme, String)> = Vec::new();
    let mut seen: Vec<(Scheme, String)> = Vec::new();

    let push = |scheme: Scheme, val: String, out: &mut Vec<(Scheme, String)>, seen: &mut Vec<(Scheme, String)>| {
        let v = val.trim().trim_end_matches(|c: char| c == '.' || c == ',' || c == ';' || c == ')').to_string();
        if v.is_empty() {
            return;
        }
        let key = (scheme, v.clone());
        if seen.contains(&key) {
            return;
        }
        seen.push(key);
        out.push((scheme, v));
    };

    // 1) DOI URL
    if let Some(c) = RE_DOI_URL.captures(s) {
        push(Scheme::Doi, c.get(1).unwrap().as_str().to_string(), &mut out, &mut seen);
    }

    // 2) arXiv URL（新格式）
    if let Some(c) = RE_ARXIV_URL.captures(s) {
        push(Scheme::Arxiv, c.get(1).unwrap().as_str().to_string(), &mut out, &mut seen);
    }

    // 3) arXiv URL（旧格式）
    if let Some(c) = RE_ARXIV_OLD_URL.captures(s) {
        push(Scheme::Arxiv, c.get(1).unwrap().as_str().to_string(), &mut out, &mut seen);
    }

    // 4) PMID URL
    if let Some(c) = RE_PMID_URL.captures(s) {
        push(Scheme::Pmid, c.get(1).unwrap().as_str().to_string(), &mut out, &mut seen);
    }

    // 5) 裸 DOI（避免 arXiv 形如 2401.01234 被错配）
    if !out.iter().any(|(sc, _)| *sc == Scheme::Doi) {
        if let Some(m) = RE_DOI.find(s) {
            push(Scheme::Doi, m.as_str().to_string(), &mut out, &mut seen);
        }
    }

    // 6) 裸 arXiv（先新后旧）；用 find + 上下文判断，
    //    避免 "CVPR.2020.01234" 中的 "2020.01234" 被误判为 arXiv。
    if !out.iter().any(|(sc, _)| *sc == Scheme::Doi) {
        let bytes = s.as_bytes();
        let mut pushed = false;
        for m in RE_ARXIV_NEW.find_iter(s) {
            if ctx_ok(bytes, m.start(), m.end()) {
                push(Scheme::Arxiv, m.as_str().to_string(), &mut out, &mut seen);
                pushed = true;
                break;
            }
        }
        if !pushed {
            for m in RE_ARXIV_OLD.find_iter(s) {
                if ctx_ok(bytes, m.start(), m.end()) {
                    push(Scheme::Arxiv, m.as_str().to_string(), &mut out, &mut seen);
                    break;
                }
            }
        }
    }

    // 7) 裸 PMID：
    //    a) 带 "PMID" 前缀 → 必为 PMID；
    //    b) 整段 input 仅是一个 ≤ 8 位数字（无其它字符）→ 视为 PMID；
    //    c) 否则保守不匹配（避免 "Published in 2024" 误判）。
    if !out.iter().any(|(sc, _)| *sc == Scheme::Pmid) {
        if let Some(c) = RE_PMID_BARE.captures(s) {
            push(Scheme::Pmid, c.get(1).unwrap().as_str().to_string(), &mut out, &mut seen);
        } else {
            // 整段数字才视作 PMID。
            let stripped = s
                .trim()
                .trim_start_matches("pmid:")
                .trim_start_matches("PMID:")
                .trim();
            if !stripped.is_empty()
                && stripped.chars().all(|c| c.is_ascii_digit())
                && stripped.len() <= 8
                && !out.iter().any(|(sc, _)| *sc == Scheme::Arxiv || *sc == Scheme::Doi)
            {
                push(Scheme::Pmid, stripped.to_string(), &mut out, &mut seen);
            }
        }
    }

    // 8) ISBN
    if !out.iter().any(|(sc, _)| *sc == Scheme::Isbn) {
        if let Some(m) = RE_ISBN.find(s) {
            let v = m.as_str().replace('-', "");
            if is_valid_isbn(&v) {
                push(Scheme::Isbn, v, &mut out, &mut seen);
            }
        }
    }

    out
}

/// 上下文合法判断：命中区间 `[start, end)` 的前后字符不能是
/// `[A-Za-z0-9.]`（用于 arXiv 命中的边界检查）。
fn ctx_ok(bytes: &[u8], start: usize, end: usize) -> bool {
    fn is_word_dot(b: u8) -> bool {
        b.is_ascii_alphanumeric() || b == b'.'
    }
    if start > 0 && is_word_dot(bytes[start - 1]) {
        return false;
    }
    if end < bytes.len() && is_word_dot(bytes[end]) {
        return false;
    }
    true
}

/// ISBN-10 / ISBN-13 校验。
pub fn is_valid_isbn(s: &str) -> bool {
    let s = s.replace('-', "").replace(' ', "");
    match s.len() {
        10 => validate_isbn10(&s),
        13 => validate_isbn13(&s),
        _ => false,
    }
}

fn validate_isbn10(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let bytes = s.as_bytes();
    let mut sum: i32 = 0;
    for (i, b) in bytes.iter().enumerate() {
        let d = match *b {
            b'0'..=b'9' => (b - b'0') as i32,
            b'X' | b'x' if i == 9 => 10,
            _ => return false,
        };
        sum += d * (10 - i as i32);
    }
    sum % 11 == 0
}

fn validate_isbn13(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() != 13 {
        return false;
    }
    let mut sum: i32 = 0;
    for (i, b) in bytes.iter().enumerate() {
        let d = match *b {
            b'0'..=b'9' => (b - b'0') as i32,
            _ => return false,
        };
        // 位置 0/2/4/... 权重 1，1/3/5/... 权重 3
        sum += if i % 2 == 0 { d } else { d * 3 };
    }
    sum % 10 == 0
}

// -----------------------------------------------------------
// Tests
// -----------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty() {
        assert!(parse("").is_empty());
        assert!(parse("   ").is_empty());
    }

    #[test]
    fn parse_plain_doi() {
        let r = parse("10.1109/CVPR.2020.01234");
        assert_eq!(r, vec![(Scheme::Doi, "10.1109/CVPR.2020.01234".into())]);
    }

    #[test]
    fn parse_doi_url_https() {
        let r = parse("https://doi.org/10.1109/CVPR.2020.01234");
        assert_eq!(r, vec![(Scheme::Doi, "10.1109/CVPR.2020.01234".into())]);
    }

    #[test]
    fn parse_doi_url_dx() {
        let r = parse("http://dx.doi.org/10.1038/nature12373");
        assert_eq!(r, vec![(Scheme::Doi, "10.1038/nature12373".into())]);
    }

    #[test]
    fn parse_doi_with_prefix() {
        let r = parse("DOI: 10.1109/foo");
        assert_eq!(r, vec![(Scheme::Doi, "10.1109/foo".into())]);
    }

    #[test]
    fn parse_arxiv_new_bare() {
        let r = parse("2401.01234");
        assert_eq!(r, vec![(Scheme::Arxiv, "2401.01234".into())]);
    }

    #[test]
    fn parse_arxiv_new_with_version() {
        let r = parse("2401.01234v2");
        assert_eq!(r, vec![(Scheme::Arxiv, "2401.01234v2".into())]);
    }

    #[test]
    fn parse_arxiv_new_url() {
        let r = parse("https://arxiv.org/abs/2401.01234");
        assert_eq!(r, vec![(Scheme::Arxiv, "2401.01234".into())]);
    }

    #[test]
    fn parse_arxiv_new_url_pdf() {
        let r = parse("https://arxiv.org/pdf/2401.01234v3");
        assert_eq!(r, vec![(Scheme::Arxiv, "2401.01234v3".into())]);
    }

    #[test]
    fn parse_arxiv_old_bare() {
        let r = parse("cs.LG/0703001");
        assert_eq!(r, vec![(Scheme::Arxiv, "cs.LG/0703001".into())]);
    }

    #[test]
    fn parse_arxiv_old_url() {
        let r = parse("https://arxiv.org/abs/cs.LG/0703001");
        assert_eq!(r, vec![(Scheme::Arxiv, "cs.LG/0703001".into())]);
    }

    #[test]
    fn parse_pmid_url() {
        let r = parse("https://pubmed.ncbi.nlm.nih.gov/12345");
        assert_eq!(r, vec![(Scheme::Pmid, "12345".into())]);
    }

    #[test]
    fn parse_pmid_with_prefix() {
        let r = parse("PMID: 12345678");
        assert_eq!(r, vec![(Scheme::Pmid, "12345678".into())]);
    }

    #[test]
    fn parse_pmid_bare_does_not_match_year() {
        // 单独 "2024" 不应被识别为 PMID（4 位年份太容易误中）
        let r = parse("Published in 2024");
        assert!(!r.iter().any(|(s, _)| *s == Scheme::Pmid), "year should not match pmid: {r:?}");
    }

    #[test]
    fn parse_pmid_pure_number_input() {
        // 整段只是数字 → 视为 PMID（≤ 8 位）
        let r = parse("12345");
        assert!(r.iter().any(|(s, _)| *s == Scheme::Pmid), "should match pmid: {r:?}");
    }

    #[test]
    fn parse_pmid_pure_number_with_text() {
        // 含其它字符 → 不视作 PMID
        let r = parse("See reference 12345678 for details");
        assert!(!r.iter().any(|(s, _)| *s == Scheme::Pmid), "embedded number should not match: {r:?}");
    }

    #[test]
    fn parse_isbn_10() {
        // ISBN-10 校验位
        let r = parse("0306406152");
        assert_eq!(r, vec![(Scheme::Isbn, "0306406152".into())]);
    }

    #[test]
    fn parse_isbn_13_with_dashes() {
        let r = parse("978-0-306-40615-7");
        assert!(r.iter().any(|(s, _)| *s == Scheme::Isbn), "should match isbn: {r:?}");
    }

    #[test]
    fn parse_isbn_invalid_check() {
        // 故意构造一个错误校验位的 ISBN-13
        let r = parse("978-0-306-40615-0");
        assert!(!r.iter().any(|(s, _)| *s == Scheme::Isbn), "invalid isbn must not match: {r:?}");
    }

    #[test]
    fn parse_priority_doi_over_arxiv() {
        // 长串里同时含 DOI 和 arXiv → DOI 优先（arxiv 不再独立输出，
        // 但 DOI 解析器返回后通常会带原 arXiv id 在 metadata 中）。
        let r = parse("DOI 10.1109/foo and arXiv:2401.01234");
        assert!(r.iter().any(|(s, _)| *s == Scheme::Doi));
        // DOI 命中后 arXiv 步骤会被跳过；resolver 阶段可从 DOI
        // 反向查 arXiv（例如 Crossref 偶尔会带 arXiv id），但
        // 解析器阶段 arXiv 不会独立进入 out。
        assert!(!r.iter().any(|(s, _)| *s == Scheme::Arxiv));
    }

    #[test]
    fn parse_does_not_match_garbage() {
        let r = parse("hello world");
        assert!(r.is_empty());
    }

    // -------------------------------------------------------
    // is_valid_isbn 单元测试
    // -------------------------------------------------------

    #[test]
    fn isbn10_check_digits() {
        // 0306406152 是合法 ISBN-10（参考 wikipedia 例）
        assert!(is_valid_isbn("0306406152"));
        assert!(is_valid_isbn("0-306-40615-2"));
        // 校验位错
        assert!(!is_valid_isbn("0306406153"));
    }

    #[test]
    fn isbn10_x_check() {
        // 155404295X 是合法 ISBN-10
        assert!(is_valid_isbn("155404295X"));
        assert!(!is_valid_isbn("155404295Y"));
    }

    #[test]
    fn isbn13_check_digits() {
        // 9780306406157 合法
        assert!(is_valid_isbn("9780306406157"));
        assert!(is_valid_isbn("978-0-306-40615-7"));
        // 校验位错
        assert!(!is_valid_isbn("9780306406150"));
    }

    #[test]
    fn isbn_invalid_length() {
        assert!(!is_valid_isbn("123"));
        assert!(!is_valid_isbn("12345678901234"));
    }
}
