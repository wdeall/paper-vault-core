//! 4 个 identifier resolver：DOI / arXiv / PMID / ISBN。
//!
//! 每个 resolver 实现 `Resolver` trait：声明自己的 scheme 并从
//! 对应外部 API 拉取元数据，统一映射到 `PaperMetadata`。
//!
//! 错误分类（用于前端友好提示）：
//!   - 404 / 解析失败：找不到该 identifier 的元数据
//!   - 401 / 403：API key / 权限
//!   - 429：源服务限流
//!   - 超时：网络请求超时
//!   - 其它：原始错误

use crate::error::{AppError, AppResult};
use crate::services::identifier::Scheme;
use crate::types::PaperMetadata;
use async_trait::async_trait;
use reqwest::Client;
use std::time::Duration;

pub const DEFAULT_TIMEOUT_SECS: u64 = 20;
pub const DEFAULT_USER_AGENT: &str = "PaperVault/0.1 (https://github.com/...)";

/// Resolver 抽象。
#[async_trait]
pub trait Resolver: Send + Sync {
    #[allow(dead_code)] // 通过 `Box<dyn Resolver>` 调用；当前 import_by_identifier 走 match，trait 入口暂未触发。
    fn scheme(&self) -> Scheme;
    /// 给定纯 value（不含 URL 前缀），返回 `PaperMetadata`。
    /// 失败时必须返回 `AppError`，由 `import_by_identifier` 转为
    /// 对应用户提示。
    async fn fetch(&self, value: &str) -> AppResult<PaperMetadata>;
}

// ============================================================
// Crossref (DOI)
// ============================================================

pub struct CrossrefResolver {
    pub client: Client,
    pub base_url: String,
}

impl CrossrefResolver {
    pub fn new() -> AppResult<Self> {
        Ok(Self {
            client: build_client()?,
            base_url: "https://api.crossref.org/works".into(),
        })
    }

    #[allow(dead_code)] // 测试构造时使用；生产代码走 new() 即可。
    pub fn with_base_url(base_url: impl Into<String>) -> AppResult<Self> {
        Ok(Self {
            client: build_client()?,
            base_url: base_url.into(),
        })
    }
}

#[async_trait]
impl Resolver for CrossrefResolver {
    fn scheme(&self) -> Scheme {
        Scheme::Doi
    }

    async fn fetch(&self, value: &str) -> AppResult<PaperMetadata> {
        let url = format!("{}/{}", self.base_url.trim_end_matches('/'), value);
        let resp = send_with_retry(&self.client, &url).await?;
        let status = resp.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(AppError::NotFound(format!(
                "找不到该 identifier 的元数据 ({})",
                value
            )));
        }
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(AppError::Other("源服务限流，请稍后重试".into()));
        }
        if !status.is_success() {
            return Err(AppError::Other(format!(
                "Crossref 返回 {}: {}",
                status,
                value
            )));
        }
        let body: serde_json::Value = resp.json().await?;
        parse_crossref_message(&body)
    }
}

fn parse_crossref_message(body: &serde_json::Value) -> AppResult<PaperMetadata> {
    let msg = body
        .get("message")
        .ok_or_else(|| AppError::Other("Crossref 响应缺 message 字段".into()))?;
    let title = msg
        .get("title")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    let authors: Vec<String> = msg
        .get("author")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|a| {
                    let fam = a.get("family").and_then(|v| v.as_str()).unwrap_or("");
                    let giv = a.get("given").and_then(|v| v.as_str()).unwrap_or("");
                    let name = if !fam.is_empty() && !giv.is_empty() {
                        format!("{giv} {fam}")
                    } else if !fam.is_empty() {
                        fam.to_string()
                    } else if !giv.is_empty() {
                        giv.to_string()
                    } else {
                        a.get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string()
                    };
                    if name.trim().is_empty() {
                        None
                    } else {
                        Some(name)
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    let year = msg
        .get("issued")
        .and_then(|v| v.get("date-parts"))
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|v| v.as_i64())
        .map(|y| y as i32);

    let venue = msg
        .get("container-title")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    let doi = msg
        .get("DOI")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    let abstract_text = msg
        .get("abstract")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        // 去掉 JATS XML 标签（Crossref 摘要常带简单 <jats:p> 等）
        .replace("<jats:p>", "")
        .replace("</jats:p>", "")
        .to_string();

    let subjects: Vec<String> = msg
        .get("subject")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let mut identifiers = Vec::new();
    if !doi.is_empty() {
        identifiers.push(("doi".to_string(), doi.clone()));
    }

    Ok(PaperMetadata {
        title,
        authors,
        year,
        venue,
        doi,
        abstract_text,
        keywords: subjects,
        identifiers,
    })
}

// ============================================================
// arXiv
// ============================================================

pub struct ArxivResolver {
    pub client: Client,
    pub base_url: String,
}

impl ArxivResolver {
    pub fn new() -> AppResult<Self> {
        Ok(Self {
            client: build_client()?,
            base_url: "https://export.arxiv.org/api/query".into(),
        })
    }

    #[allow(dead_code)]
    pub fn with_base_url(base_url: impl Into<String>) -> AppResult<Self> {
        Ok(Self {
            client: build_client()?,
            base_url: base_url.into(),
        })
    }
}

#[async_trait]
impl Resolver for ArxivResolver {
    fn scheme(&self) -> Scheme {
        Scheme::Arxiv
    }

    async fn fetch(&self, value: &str) -> AppResult<PaperMetadata> {
        let url = format!("{}?id_list={}", self.base_url, value);
        let resp = send_with_retry(&self.client, &url).await?;
        let status = resp.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(AppError::Other("源服务限流，请稍后重试".into()));
        }
        if !status.is_success() {
            return Err(AppError::Other(format!(
                "arXiv 返回 {}: {}",
                status, value
            )));
        }
        let body = resp.text().await?;
        parse_arxiv_atom(&body)
    }
}

fn parse_arxiv_atom(xml: &str) -> AppResult<PaperMetadata> {
    use regex::Regex;
    use std::sync::OnceLock;

    fn re() -> &'static Regex {
        static R: OnceLock<Regex> = OnceLock::new();
        R.get_or_init(|| Regex::new(r"(?s)<entry>(.*?)</entry>").unwrap())
    }

    let entry = re()
        .captures(xml)
        .ok_or_else(|| AppError::NotFound("arXiv 响应中无 entry".into()))?
        .get(1)
        .unwrap()
        .as_str()
        .to_string();

    if entry.contains("<id>http://arxiv.org/error") {
        return Err(AppError::NotFound("arXiv 找不到该 identifier".into()));
    }

    let title = extract_first(&entry, "title").unwrap_or_default().trim().to_string();
    let summary = extract_first(&entry, "summary").unwrap_or_default().trim().to_string();
    let published = extract_first(&entry, "published").unwrap_or_default();
    let year = published.get(..4).and_then(|s| s.parse::<i32>().ok());

    let authors: Vec<String> = {
        static R: OnceLock<Regex> = OnceLock::new();
        let r = R.get_or_init(|| Regex::new(r"(?s)<author>\s*<name>(.*?)</name>").unwrap());
        r.captures_iter(&entry)
            .map(|c| c.get(1).unwrap().as_str().trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    };

    let doi = {
        static R: OnceLock<Regex> = OnceLock::new();
        let r = R.get_or_init(|| Regex::new(r"(?s)<arxiv:doi[^>]*>(.*?)</arxiv:doi>").unwrap());
        r.captures(&entry)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_default()
    };
    let journal_ref = extract_first(&entry, "journal_ref").unwrap_or_default().trim().to_string();
    let venue = if !journal_ref.is_empty() { journal_ref } else { "arXiv".into() };

    let mut identifiers = Vec::new();
    // arXiv id 自己
    if let Some(id) = extract_first(&entry, "id") {
        // 形如 http://arxiv.org/abs/2401.01234v2 → 取末尾
        let id_short = id.rsplit('/').next().unwrap_or(id.trim()).to_string();
        if !id_short.is_empty() {
            identifiers.push(("arxiv".to_string(), id_short));
        }
    }
    if !doi.is_empty() {
        identifiers.push(("doi".to_string(), doi.clone()));
    }

    Ok(PaperMetadata {
        title,
        authors,
        year,
        venue,
        doi,
        abstract_text: summary,
        keywords: Vec::new(),
        identifiers,
    })
}

fn extract_first(xml: &str, tag: &str) -> Option<String> {
    use regex::Regex;
    use std::sync::OnceLock;
    static CACHE: OnceLock<std::sync::Mutex<Vec<(String, Regex)>>> = OnceLock::new();
    let mut cache = CACHE.get_or_init(|| std::sync::Mutex::new(Vec::new())).lock().ok()?;
    let pattern = format!(r"(?s)<(?:[a-zA-Z][a-zA-Z0-9_-]*:)?{tag}(?:\s[^>]*)?>(.*?)</(?:[a-zA-Z][a-zA-Z0-9_-]*:)?{tag}>");
    let re = if let Some((_, r)) = cache.iter().find(|(k, _)| k == &pattern) {
        r
    } else {
        let r = Regex::new(&pattern).ok()?;
        cache.push((pattern, r));
        cache.last().map(|(_, r)| r).unwrap()
    };
    re.captures(xml)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

// ============================================================
// PubMed (PMID)
// ============================================================

pub struct PubMedResolver {
    pub client: Client,
    pub base_url: String,
}

impl PubMedResolver {
    pub fn new() -> AppResult<Self> {
        Ok(Self {
            client: build_client()?,
            base_url: "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esummary.fcgi".into(),
        })
    }
}

#[async_trait]
impl Resolver for PubMedResolver {
    fn scheme(&self) -> Scheme {
        Scheme::Pmid
    }

    async fn fetch(&self, value: &str) -> AppResult<PaperMetadata> {
        let url = format!(
            "{}?bibkeys=ISBN:{}&format=json&jscmd=data",
            self.base_url, value
        );
        let resp = send_with_retry(&self.client, &url).await?;
        let status = resp.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(AppError::NotFound(format!(
                "找不到该 PMID 对应的元数据 ({})",
                value
            )));
        }
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(AppError::Other("源服务限流，请稍后重试".into()));
        }
        if !status.is_success() {
            return Err(AppError::Other(format!(
                "PubMed 返回 {}: {}",
                status, value
            )));
        }
        let body: serde_json::Value = resp.json().await?;
        parse_pubmed_summary(&body, value)
    }
}

fn parse_pubmed_summary(body: &serde_json::Value, pmid: &str) -> AppResult<PaperMetadata> {
    let result = body
        .get("result")
        .ok_or_else(|| AppError::Other("PubMed 响应缺 result".into()))?;
    let rec = result
        .get(pmid)
        .ok_or_else(|| AppError::NotFound("PubMed 找不到该 identifier".into()))?;
    let title = rec
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let authors: Vec<String> = rec
        .get("authors")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|a| {
                    a.get("name")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                })
                .collect()
        })
        .unwrap_or_default();
    let venue = rec
        .get("fulljournalname")
        .or_else(|| rec.get("source"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let year = rec
        .get("pubdate")
        .and_then(|v| v.as_str())
        .and_then(|s| s.get(..4))
        .and_then(|s| s.parse::<i32>().ok());
    let articleids = rec
        .get("articleids")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut doi = String::new();
    for id in &articleids {
        let idtype = id.get("idtype").and_then(|v| v.as_str()).unwrap_or("");
        let val = id.get("value").and_then(|v| v.as_str()).unwrap_or("");
        if idtype == "doi" {
            doi = val.trim().to_string();
            break;
        }
    }
    let mut identifiers = Vec::new();
    identifiers.push(("pmid".to_string(), pmid.to_string()));
    if !doi.is_empty() {
        identifiers.push(("doi".to_string(), doi.clone()));
    }
    Ok(PaperMetadata {
        title,
        authors,
        year,
        venue,
        doi,
        // PubMed esummary 不返回 abstract；如需 abstract 应调 efetch。
        // 本期只填 esummary，abstract 留空，由后续 P 阶段 / AI 拉取补齐。
        abstract_text: String::new(),
        keywords: Vec::new(),
        identifiers,
    })
}

// ============================================================
// OpenLibrary (ISBN)
// ============================================================

pub struct OpenLibraryResolver {
    pub client: Client,
    pub base_url: String,
}

impl OpenLibraryResolver {
    pub fn new() -> AppResult<Self> {
        Ok(Self {
            client: build_client()?,
            base_url: "https://openlibrary.org/api/books".into(),
        })
    }
}

#[async_trait]
impl Resolver for OpenLibraryResolver {
    fn scheme(&self) -> Scheme {
        Scheme::Isbn
    }

    async fn fetch(&self, value: &str) -> AppResult<PaperMetadata> {
        let url = format!(
            "{}?bibkeys=ISBN:{}&format=json&jscmd=data",
            self.base_url, value
        );
        let resp = send_with_retry(&self.client, &url).await?;
        let status = resp.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(AppError::NotFound(format!(
                "找不到该 ISBN 对应的元数据 ({})",
                value
            )));
        }
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(AppError::Other("源服务限流，请稍后重试".into()));
        }
        if !status.is_success() {
            return Err(AppError::Other(format!(
                "OpenLibrary 返回 {}: {}",
                status, value
            )));
        }
        let body: serde_json::Value = resp.json().await?;
        let key = format!("ISBN:{value}");
        let rec = body
            .get(&key)
            .ok_or_else(|| AppError::NotFound("OpenLibrary 找不到该 ISBN".into()))?;
        parse_openlibrary_book(rec, value)
    }
}

fn parse_openlibrary_book(rec: &serde_json::Value, isbn: &str) -> AppResult<PaperMetadata> {
    let title = rec
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let authors: Vec<String> = rec
        .get("authors")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|a| a.get("name").and_then(|v| v.as_str()).map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let year = rec
        .get("publish_date")
        .and_then(|v| v.as_str())
        .and_then(|s| {
            // 形如 "March 2010" / "2010" / "2010-01-01"
            let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
            if digits.len() >= 4 {
                digits[..4].parse::<i32>().ok()
            } else {
                None
            }
        });
    let venue = rec
        .get("publishers")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|a| a.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let identifiers = vec![("isbn".to_string(), isbn.to_string())];
    Ok(PaperMetadata {
        title,
        authors,
        year,
        venue,
        doi: String::new(),
        abstract_text: String::new(),
        keywords: Vec::new(),
        identifiers,
    })
}

// ============================================================
// helpers
// ============================================================

fn build_client() -> AppResult<Client> {
    Client::builder()
        .user_agent(DEFAULT_USER_AGENT)
        .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
        .build()
        .map_err(|e| AppError::Other(format!("HTTP client 初始化失败: {e}")))
}

/// 包装 `client.get(url).send()`，对网络层 transient error 重试一次。
/// SPEC §7.2 承诺 "网络错误重试一次"。
/// 4xx / 5xx / 解析错误不在重试范围（重试 4xx 没意义，重试 5xx 留给客户端 middleware）。
async fn send_with_retry(client: &Client, url: &str) -> AppResult<reqwest::Response> {
    let r1 = client.get(url).send().await;
    match r1 {
        Ok(r) => Ok(r),
        Err(e) => {
            log::warn!("resolver transient error: {e}; retrying once");
            client.get(url).send().await.map_err(AppError::from)
        }
    }
}

/// 工厂：根据 scheme 拿到对应 resolver。
#[allow(dead_code)] // 当前 import_by_identifier 走 match，工厂暂未在生产路径触发；保留供 M-C 搜索阶段切换为 dyn dispatch。
pub fn default_resolver(scheme: Scheme) -> AppResult<Box<dyn Resolver>> {
    match scheme {
        Scheme::Doi => Ok(Box::new(CrossrefResolver::new()?)),
        Scheme::Arxiv => Ok(Box::new(ArxivResolver::new()?)),
        Scheme::Pmid => Ok(Box::new(PubMedResolver::new()?)),
        Scheme::Isbn => Ok(Box::new(OpenLibraryResolver::new()?)),
    }
}

/// 按标题在 Crossref 模糊搜索，返回最匹配的前若干条元数据。
/// 用于 PDF 抽不到 DOI 时按标题反查网络元数据。
///
/// 调用 Crossref `/works?query.bibliographic={title}&rows={limit}`。
/// 返回结果按 Crossref 相关性排序，调用方可取第一个作为最佳匹配。
pub async fn search_by_title(title: &str, limit: usize) -> AppResult<Vec<PaperMetadata>> {
    if title.trim().is_empty() {
        return Ok(Vec::new());
    }
    let client = build_client()?;
    let url = format!(
        "https://api.crossref.org/works?query.bibliographic={}&rows={}",
        urlencode(title),
        limit.max(1).min(10)
    );
    let resp = send_with_retry(&client, &url).await?;
    let status = resp.status();
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(AppError::Other("Crossref 限流，请稍后重试".into()));
    }
    if !status.is_success() {
        return Err(AppError::Other(format!(
            "Crossref 搜索失败: HTTP {status}"
        )));
    }
    let body: serde_json::Value = resp.json().await?;
    let items = body
        .get("message")
        .and_then(|m| m.get("items"))
        .and_then(|i| i.as_array())
        .ok_or_else(|| AppError::Other("Crossref 搜索响应缺 message.items".into()))?;
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        // 复用 parse_crossref_message：它期望 { "message": { ... } }，
        // 这里每个 item 本身就是 message 结构，包一层即可。
        let wrapped = serde_json::json!({ "message": item });
        match parse_crossref_message(&wrapped) {
            Ok(m) => out.push(m),
            Err(e) => log::warn!("Crossref 搜索结果解析失败: {e}"),
        }
    }
    Ok(out)
}

/// 简易 URL 编码（仅编码查询参数中的特殊字符）。
fn urlencode(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            ' ' => "+".into(),
            c if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '~' => {
                c.to_string()
            }
            c => format!("%{:02X}", c as u8),
        })
        .collect()
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn crossref_parses_minimal_message() {
        let server = MockServer::start().await;
        // production base_url = "{api.crossref.org}/works"，mock 用
        // "{server.uri()}/works" 才能 path 匹配。
        let base = format!("{}/works", server.uri());
        Mock::given(method("GET"))
            .and(path("/works/10.1109/foo"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "status": "ok",
                "message": {
                    "DOI": "10.1109/foo",
                    "title": ["A Sample Paper"],
                    "author": [
                        {"given": "Alice", "family": "Smith"},
                        {"given": "Bob", "family": "Jones"}
                    ],
                    "container-title": ["Nature"],
                    "issued": {"date-parts": [[2024, 3]]},
                    "abstract": "<jats:p>An <jats:bold>abstract</jats:bold>.</jats:p>",
                    "subject": ["Computer Science", "ML"]
                }
            })))
            .mount(&server)
            .await;

        let r = CrossrefResolver::with_base_url(base).unwrap();
        let m = r.fetch("10.1109/foo").await.unwrap();
        assert_eq!(m.title, "A Sample Paper");
        assert_eq!(m.authors, vec!["Alice Smith", "Bob Jones"]);
        assert_eq!(m.year, Some(2024));
        assert_eq!(m.venue, "Nature");
        assert_eq!(m.doi, "10.1109/foo");
        assert!(m.abstract_text.contains("abstract"));
        assert_eq!(m.keywords, vec!["Computer Science", "ML"]);
        assert!(m.identifiers.iter().any(|(k, v)| k == "doi" && v == "10.1109/foo"));
    }

    #[tokio::test]
    async fn crossref_404_returns_not_found() {
        let server = MockServer::start().await;
        let base = format!("{}/works", server.uri());
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let r = CrossrefResolver::with_base_url(base).unwrap();
        let err = r.fetch("10.1109/missing").await.unwrap_err();
        assert!(matches!(err, AppError::NotFound(_)), "got {err:?}");
    }

    #[tokio::test]
    async fn arxiv_parses_atom_entry() {
        let body = r#"<?xml version="1.0"?>
<feed xmlns="http://www.w3.org/2005/Atom" xmlns:arxiv="http://arxiv.org/schemas/atom">
  <entry>
    <id>http://arxiv.org/abs/2401.01234v2</id>
    <title> An Interesting Paper </title>
    <summary>We study things.</summary>
    <published>2024-01-15T00:00:00Z</published>
    <author><name>Alice Smith</name></author>
    <author><name>Bob Jones</name></author>
    <arxiv:doi>10.1109/foo</arxiv:doi>
    <arxiv:journal_ref>Nature Machine Intelligence</arxiv:journal_ref>
  </entry>
</feed>"#;
        let m = parse_arxiv_atom(body).unwrap();
        assert_eq!(m.title, "An Interesting Paper");
        assert_eq!(m.authors, vec!["Alice Smith", "Bob Jones"]);
        assert_eq!(m.year, Some(2024));
        assert_eq!(m.venue, "Nature Machine Intelligence");
        assert_eq!(m.doi, "10.1109/foo");
        assert_eq!(m.abstract_text, "We study things.");
        assert!(m.identifiers.iter().any(|(k, v)| k == "arxiv" && v == "2401.01234v2"));
        assert!(m.identifiers.iter().any(|(k, v)| k == "doi" && v == "10.1109/foo"));
    }

    #[tokio::test]
    async fn arxiv_error_entry_returns_not_found() {
        let body = r#"<?xml version="1.0"?>
<feed>
  <entry>
    <id>http://arxiv.org/error</id>
    <title>Error</title>
    <summary>bad id</summary>
  </entry>
</feed>"#;
        let err = parse_arxiv_atom(body).unwrap_err();
        assert!(matches!(err, AppError::NotFound(_)), "got {err:?}");
    }

    #[test]
    fn pubmed_parses_minimal_summary() {
        let body = json!({
            "result": {
                "12345": {
                    "title": "A PMID Paper",
                    "authors": [{"name": "Alice"}, {"name": "Bob"}],
                    "fulljournalname": "Cell",
                    "pubdate": "2023 May",
                    "articleids": [
                        {"idtype": "pubmed", "value": "12345"},
                        {"idtype": "doi", "value": "10.1109/bar"}
                    ]
                }
            }
        });
        let m = parse_pubmed_summary(&body, "12345").unwrap();
        assert_eq!(m.title, "A PMID Paper");
        assert_eq!(m.authors, vec!["Alice", "Bob"]);
        assert_eq!(m.venue, "Cell");
        assert_eq!(m.year, Some(2023));
        assert_eq!(m.doi, "10.1109/bar");
        assert!(m.identifiers.iter().any(|(k, v)| k == "pmid" && v == "12345"));
        assert!(m.identifiers.iter().any(|(k, v)| k == "doi" && v == "10.1109/bar"));
    }

    #[test]
    fn pubmed_missing_record() {
        let body = json!({"result": {}});
        let err = parse_pubmed_summary(&body, "99999").unwrap_err();
        assert!(matches!(err, AppError::NotFound(_)));
    }

    #[test]
    fn openlibrary_parses_minimal_book() {
        let body = json!({
            "ISBN:9780123456789": {
                "title": "A Book",
                "authors": [{"name": "Alice"}, {"name": "Bob"}],
                "publish_date": "2020",
                "publishers": [{"name": "Acme Press"}]
            }
        });
        let rec = body.get("ISBN:9780123456789").unwrap();
        let m = parse_openlibrary_book(rec, "9780123456789").unwrap();
        assert_eq!(m.title, "A Book");
        assert_eq!(m.authors, vec!["Alice", "Bob"]);
        assert_eq!(m.year, Some(2020));
        assert_eq!(m.venue, "Acme Press");
        assert!(m.identifiers.iter().any(|(k, v)| k == "isbn" && v == "9780123456789"));
    }

    #[test]
    fn openlibrary_publish_date_year_extraction() {
        // "March 2010"  → 2010
        let body = json!({"ISBN:x": {"publish_date": "March 2010"}});
        let m = parse_openlibrary_book(body.get("ISBN:x").unwrap(), "x").unwrap();
        assert_eq!(m.year, Some(2010));
        // "2010-01-01" → 2010
        let body = json!({"ISBN:x": {"publish_date": "2010-01-01"}});
        let m = parse_openlibrary_book(body.get("ISBN:x").unwrap(), "x").unwrap();
        assert_eq!(m.year, Some(2010));
    }

    #[tokio::test]
    async fn pubmed_fetches_minimal_summary() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "result": {
                    "12345": {
                        "title": "Test PMID Paper",
                        "authors": [{"name": "Alice"}],
                        "fulljournalname": "Nature",
                        "pubdate": "2024 Jan 5",
                        "articleids": [
                            {"idtype": "pubmed", "value": "12345"},
                            {"idtype": "doi", "value": "10.1109/test"}
                        ]
                    }
                }
            })))
            .mount(&server)
            .await;
        let r = PubMedResolver {
            client: build_client().unwrap(),
            base_url: server.uri(),
        };
        let m = r.fetch("12345").await.unwrap();
        assert_eq!(m.title, "Test PMID Paper");
        assert_eq!(m.authors, vec!["Alice"]);
        assert_eq!(m.venue, "Nature");
        assert_eq!(m.year, Some(2024));
        assert_eq!(m.doi, "10.1109/test");
        assert!(m.identifiers.iter().any(|(k, v)| k == "pmid" && v == "12345"));
    }

    #[tokio::test]
    async fn pubmed_404_returns_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let r = PubMedResolver {
            client: build_client().unwrap(),
            base_url: server.uri(),
        };
        let err = r.fetch("99999").await.unwrap_err();
        assert!(matches!(err, AppError::NotFound(_)), "got {err:?}");
    }

    #[tokio::test]
    async fn openlibrary_fetches_minimal_book() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "ISBN:9780123456789": {
                    "title": "Test Book",
                    "authors": [{"name": "Alice"}],
                    "publish_date": "2020",
                    "publishers": [{"name": "Test Press"}]
                }
            })))
            .mount(&server)
            .await;
        let r = OpenLibraryResolver {
            client: build_client().unwrap(),
            base_url: server.uri(),
        };
        let m = r.fetch("9780123456789").await.unwrap();
        assert_eq!(m.title, "Test Book");
        assert_eq!(m.authors, vec!["Alice"]);
        assert_eq!(m.year, Some(2020));
    }

    #[tokio::test]
    async fn openlibrary_404_returns_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let r = OpenLibraryResolver {
            client: build_client().unwrap(),
            base_url: server.uri(),
        };
        let err = r.fetch("9999999999").await.unwrap_err();
        assert!(matches!(err, AppError::NotFound(_)), "got {err:?}");
    }

    #[test]
    fn default_resolver_dispatch() {
        assert_eq!(default_resolver(Scheme::Doi).unwrap().scheme(), Scheme::Doi);
        assert_eq!(default_resolver(Scheme::Arxiv).unwrap().scheme(), Scheme::Arxiv);
        assert_eq!(default_resolver(Scheme::Pmid).unwrap().scheme(), Scheme::Pmid);
        assert_eq!(default_resolver(Scheme::Isbn).unwrap().scheme(), Scheme::Isbn);
    }
}
