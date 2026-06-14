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
}

/// 提取首页文本、DOI、标题启发式、页数。
pub fn extract_basic(path: &Path) -> AppResult<BasicMeta> {
    let text_all = pdf_extract::extract_text(path).map_err(|e| AppError::Pdf(e.to_string()))?;
    let first_page_text = text_all
        .split('\x0c')
        .next()
        .unwrap_or("")
        .to_string();

    // 页数：form feed 字符数 + 1
    let page_count = text_all.matches('\x0c').count() as i32 + 1;

    // DOI 正则
    let doi_re = Regex::new(r"10\.\d{4,9}/[-._;()/:A-Z0-9]+").unwrap();
    let doi = doi_re
        .find(&first_page_text)
        .map(|m| m.as_str().to_string())
        .unwrap_or_default();

    // 标题启发式：取第一行非空、长度 8..200 的内容
    let title = first_page_text
        .lines()
        .map(|l| l.trim())
        .find(|l| l.len() >= 8 && l.len() <= 200)
        .unwrap_or("")
        .to_string();

    Ok(BasicMeta {
        title,
        doi,
        first_page_text,
        page_count,
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    // 注意：这里不写具体 PDF 解析测试，依赖 fixture 后续补。
    #[test]
    fn regex_finds_doi() {
        let re = Regex::new(r"10\.\d{4,9}/[-._;()/:A-Z0-9]+").unwrap();
        assert!(re.is_match("The DOI is 10.1109/CVPR.2020.01234."));
    }
}
