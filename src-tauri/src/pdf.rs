//! PDF 元数据 / 文本提取
//!
//! v1 只做基础识别：首页文本、DOI 正则、页数。

use crate::error::{AppError, AppResult};
use regex::Regex;
use std::path::Path;

#[derive(Debug, Default, Clone)]
pub struct BasicMeta {
    pub title: String,
    pub doi: String,
    pub first_page_text: String,
    pub page_count: i32,
    /// 从 PDF 首页/前 2 页识别出的摘要文本（已去除 "Abstract" 标记）。
    pub abstract_text: String,
    /// 从 PDF 首页识别出的关键词列表（已分词，去除 "Keywords" 标记）。
    pub keywords: Vec<String>,
}

/// 提取首页文本、DOI、标题启发式、页数。
///
/// DOI 提取策略（参考 Zotero）：
/// 1. 首页正则扫描（多数论文 DOI 在首页页眉/页脚）
/// 2. 前 3 页扫描（部分论文 DOI 在版权页）
/// 3. 全文兜底扫描（极少数论文 DOI 埋在正文引用中）
/// 优先返回最早出现的合法 DOI，避免误抓引用列表中的他人 DOI。
pub fn extract_basic(path: &Path) -> AppResult<BasicMeta> {
    let text_all = pdf_extract::extract_text(path).map_err(|e| AppError::Pdf(e.to_string()))?;
    let pages: Vec<&str> = text_all.split('\x0c').collect();
    let first_page_text = pages.first().copied().unwrap_or("").to_string();

    // 页数：form feed 字符数 + 1
    let page_count = text_all.matches('\x0c').count() as i32 + 1;

    // DOI 正则（允许大小写，与 services/identifier.rs 保持一致）
    let doi_re = Regex::new(r"10\.\d{4,9}/[-._;()/:A-Za-z0-9]+").unwrap();

    // 分级扫描：首页 → 前 3 页 → 全文。返回首个命中。
    let doi = doi_re
        .find(&first_page_text)
        .map(|m| m.as_str().to_string())
        .or_else(|| {
            // 前 3 页（含首页），扫描更早出现的位置
            let head = pages.iter().take(3).cloned().collect::<Vec<_>>().join("\n\n");
            doi_re.find(&head).map(|m| m.as_str().to_string())
        })
        .or_else(|| {
            // 全文兜底（最后防线）
            doi_re.find(&text_all).map(|m| m.as_str().to_string())
        })
        .unwrap_or_default();

    // 标题启发式：从首页文本中提取最可能是论文标题的行。
    // 策略：过滤掉明显不是标题的行（页眉、期刊名、作者、邮箱、日期、版权等），
    // 然后选择第一个长度 15..200 的行。标题通常比作者行长，比页眉短。
    let title = first_page_text
        .lines()
        .map(|l| l.trim())
        .filter(|l| l.len() >= 15 && l.len() <= 200)
        .find(|l| {
            // 过滤常见噪声行
            let lower = l.to_lowercase();
            // 含 @ 的是邮箱
            if l.contains('@') { return false; }
            // 纯数字（页码、年份）
            if l.chars().all(|c| c.is_ascii_digit()) { return false; }
            // 含 "http" / "www" 的是 URL
            if lower.contains("http") || lower.contains("www.") { return false; }
            // 含 "copyright" / "©" / "all rights reserved" 是版权行
            if lower.contains("copyright") || l.contains('©') || lower.contains("all rights reserved") { return false; }
            // 含 "received" / "accepted" / "published" 的是投稿日期行
            if lower.contains("received") || lower.contains("accepted") || lower.contains("published") { return false; }
            // 含 "vol." / "pp." / "doi" 的是期刊引用行
            if lower.contains("vol.") || lower.contains("pp.") || lower.contains("doi") { return false; }
            // 全大写短词（期刊缩写如 "IEEE", "ACM"）
            if l.len() <= 10 && l.chars().all(|c| c.is_ascii_uppercase() || c == ' ') { return false; }
            // 含逗号+数字的可能是作者列表（如 "Smith, J., Jones, A."）
            if l.matches(',').count() >= 3 && l.chars().any(|c| c.is_ascii_digit()) { return false; }
            true
        })
        .unwrap_or_else(|| {
            // 兜底：取第一个长度 8..200 的行
            first_page_text
                .lines()
                .map(|l| l.trim())
                .find(|l| l.len() >= 8 && l.len() <= 200)
                .unwrap_or("")
        })
        .to_string();

    // 摘要提取：在首页 + 第 2 页文本中查找 "Abstract" / "ABSTRACT" 标记，
    // 取标记后到下一个章节标题（Introduction/Keywords/1./I. 等）之间的文本。
    let search_text = if pages.len() >= 2 {
        format!("{}\n\n{}", first_page_text, pages.get(1).unwrap_or(&""))
    } else {
        first_page_text.clone()
    };
    let abstract_text = extract_abstract(&search_text);

    // 关键词提取：查找 "Keywords" / "Key words" / "Index Terms" 标记，
    // 取标记后到换行/句号的内容，按分号/逗号分词。
    let keywords = extract_keywords(&search_text);

    Ok(BasicMeta {
        title,
        doi,
        first_page_text,
        page_count,
        abstract_text,
        keywords,
    })
}

/// 从文本中提取摘要。识别 "Abstract" / "ABSTRACT" 标记，取到下一个章节标题。
fn extract_abstract(text: &str) -> String {
    // 查找 "Abstract" 标记（大小写不敏感，可能带冒号或换行）
    let abstract_re = Regex::new(r"(?i)(?:^|\n)\s*(?:Abstract|ABSTRACT)\s*[:：]?\s*\n?").unwrap();
    let start = match abstract_re.find(text) {
        Some(m) => m.end(),
        None => return String::new(),
    };

    // 从 start 开始查找下一个章节标题
    let rest = &text[start..];
    let section_re = Regex::new(
        r"(?im)^\s*(?:\d+\.?\s+|I+\.?\s+|Keywords?\s*[:：]|Key\s+words\s*[:：]|Index\s+Terms\s*[:：]|Introduction|1\.\s+Introduction|BACKGROUND|RELATED\s+WORK|METHODS?|RESULTS?|DISCUSSION|CONCLUSION)",
    )
    .unwrap();

    let end = section_re.find(rest).map(|m| m.start()).unwrap_or(rest.len());
    let abstract_content = rest[..end].trim();

    // 清理：去掉首尾空白行，限制长度（避免误抓整页）
    if abstract_content.is_empty() || abstract_content.len() < 30 {
        return String::new();
    }
    // 截断到合理长度（摘要通常 100-2000 字符）
    if abstract_content.len() > 3000 {
        abstract_content[..3000].trim().to_string()
    } else {
        abstract_content.to_string()
    }
}

/// 从文本中提取关键词。识别 "Keywords" / "Key words" / "Index Terms" 标记。
fn extract_keywords(text: &str) -> Vec<String> {
    // 查找关键词标记
    let kw_re = Regex::new(r"(?im)(?:Keywords?|Key\s+words|Index\s+Terms)\s*[:：—\-]\s*").unwrap();
    let start = match kw_re.find(text) {
        Some(m) => m.end(),
        None => return Vec::new(),
    };

    // 取标记后到换行或句号的内容
    let rest = &text[start..];
    let end = rest
        .find('\n')
        .map(|i| i)
        .or_else(|| rest.find('.').map(|i| i + 1))
        .unwrap_or(rest.len().min(500));
    let kw_text = rest[..end].trim();

    if kw_text.is_empty() {
        return Vec::new();
    }

    // 按分号、逗号、顿号分词
    kw_text
        .split([';', ',', '、', '·'])
        .map(|s| s.trim().trim_matches(['.', ':', '—', '-', ' '].as_ref()))
        .filter(|s| !s.is_empty() && s.len() >= 2 && s.len() <= 80)
        .map(|s| s.to_string())
        .collect()
}

/// 分页提取文本。返回 (页号从 1 开始, 文本)。
pub fn extract_pages(path: &Path) -> Vec<(i32, String)> {
    let text = match pdf_extract::extract_text(path) {
        Ok(t) => t,
        Err(e) => {
            log::warn!("PDF 文本提取失败: {e}");
            return Vec::new();
        }
    };
    text.split('\x0c')
        .enumerate()
        .filter(|(_, p)| !p.trim().is_empty())
        .map(|(i, p)| ((i + 1) as i32, p.to_string()))
        .collect()
}

/// 把 PDF 全文转为 Markdown 文件并保存到指定路径。
/// 每页用 `## 第 N 页` 标题分隔，便于后续按页定位。
/// 返回最终写入的文件路径。
pub fn save_fulltext_md(pdf_path: &Path, md_path: &Path) -> AppResult<std::path::PathBuf> {
    let pages = extract_pages(pdf_path);
    if pages.is_empty() {
        return Err(AppError::Pdf("PDF 全文提取为空，无法生成 md".into()));
    }
    if let Some(p) = md_path.parent() {
        std::fs::create_dir_all(p)?;
    }
    let mut content = String::new();
    content.push_str(&format!("# PDF 全文（共 {} 页）\n\n", pages.len()));
    for (page_no, text) in &pages {
        content.push_str(&format!("## 第 {} 页\n\n{}\n\n", page_no, text));
    }
    std::fs::write(md_path, content)?;
    Ok(md_path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    // 注意：这里不写具体 PDF 解析测试，依赖 fixture 后续补。
    #[test]
    fn regex_finds_doi() {
        let re = Regex::new(r"10\.\d{4,9}/[-._;()/:A-Za-z0-9]+").unwrap();
        assert!(re.is_match("The DOI is 10.1109/CVPR.2020.01234."));
        assert!(re.is_match("Lowercase DOI: 10.1109/cvpr.2020.01234"));
    }

    #[test]
    fn extract_abstract_basic() {
        let text = "Some title\n\nAbstract\nThis is the paper abstract. It contains multiple sentences. We propose a novel method.\n\n1. Introduction\nThe intro...";
        let result = extract_abstract(text);
        assert!(result.contains("propose a novel method"));
        assert!(!result.contains("Introduction"));
    }

    #[test]
    fn extract_abstract_with_colon() {
        let text = "Title\n\nAbstract: We study the impact of X on Y. Results show significant improvement.\n\nKeywords: ML, NLP\n";
        let result = extract_abstract(text);
        assert!(result.contains("impact of X on Y"));
    }

    #[test]
    fn extract_abstract_empty_when_no_marker() {
        let text = "Just some text without abstract marker here.";
        let result = extract_abstract(text);
        assert!(result.is_empty());
    }

    #[test]
    fn extract_keywords_basic() {
        let text = "Title\n\nAbstract: ...\n\nKeywords: machine learning; neural networks, NLP\n\n1. Introduction";
        let result = extract_keywords(text);
        assert_eq!(result.len(), 3);
        assert!(result.contains(&"machine learning".to_string()));
        assert!(result.contains(&"neural networks".to_string()));
        assert!(result.contains(&"NLP".to_string()));
    }

    #[test]
    fn extract_keywords_index_terms() {
        let text = "Index Terms— Deep learning, transformers, attention.\n\n1. Introduction";
        let result = extract_keywords(text);
        assert!(result.iter().any(|k| k.contains("Deep learning")));
    }

    #[test]
    fn extract_keywords_empty_when_no_marker() {
        let text = "Just some text without keywords marker.";
        let result = extract_keywords(text);
        assert!(result.is_empty());
    }
}
