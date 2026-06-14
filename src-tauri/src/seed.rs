//! 示例数据：3-5 篇占位论文，演示模式使用。

use crate::db;
use crate::markdown;
use crate::types::Paper;
use rusqlite::params;
use std::path::Path;

pub fn load(vault: &Path) -> anyhow::Result<Vec<String>> {
    let now = chrono::Local::now().timestamp_millis();
    let examples = vec![
        Paper {
            id: "demo001attention".into(),
            title: "Attention Is All You Need".into(),
            authors: vec!["Vaswani A".into(), "Shazeer N".into()],
            year: Some(2017),
            venue: "NeurIPS".into(),
            doi: "10.48550/arXiv.1706.03762".into(),
            abstract_text: "提出 Transformer 架构，用自注意力替代 RNN/CNN。".into(),
            keywords: vec!["transformer".into(), "self-attention".into()],
            tags: vec!["已读".into()],
            status: "已读".into(),
            rating: Some(5),
            pdf_path: String::new(),
            note_path: String::new(),
            created_at: now,
            updated_at: now,
        },
        Paper {
            id: "demo002bert".into(),
            title: "BERT: Pre-training of Deep Bidirectional Transformers".into(),
            authors: vec!["Devlin J".into()],
            year: Some(2019),
            venue: "NAACL".into(),
            doi: "10.18653/v1/N19-1423".into(),
            abstract_text: "基于 Transformer 的双向预训练语言模型。".into(),
            keywords: vec!["BERT".into(), "pretraining".into()],
            tags: vec!["阅读中".into()],
            status: "阅读中".into(),
            rating: Some(4),
            pdf_path: String::new(),
            note_path: String::new(),
            created_at: now,
            updated_at: now,
        },
        Paper {
            id: "demo003gpt".into(),
            title: "Language Models are Few-Shot Learners".into(),
            authors: vec!["Brown T B".into()],
            year: Some(2020),
            venue: "NeurIPS".into(),
            doi: "10.48550/arXiv.2005.14165".into(),
            abstract_text: "GPT-3，175B 参数，在 few-shot 上展现强大能力。".into(),
            keywords: vec!["GPT-3".into(), "in-context learning".into()],
            tags: vec!["待读".into()],
            status: "未读".into(),
            rating: None,
            pdf_path: String::new(),
            note_path: String::new(),
            created_at: now,
            updated_at: now,
        },
        Paper {
            id: "demo004vit".into(),
            title: "An Image is Worth 16x16 Words".into(),
            authors: vec!["Dosovitskiy A".into()],
            year: Some(2021),
            venue: "ICLR".into(),
            doi: "10.48550/arXiv.2010.11929".into(),
            abstract_text: "Vision Transformer，把图像切成 patch 序列输入 Transformer。".into(),
            keywords: vec!["ViT".into(), "vision transformer".into()],
            tags: vec!["阅读中".into()],
            status: "阅读中".into(),
            rating: Some(4),
            pdf_path: String::new(),
            note_path: String::new(),
            created_at: now,
            updated_at: now,
        },
        Paper {
            id: "demo005diffusion".into(),
            title: "Denoising Diffusion Probabilistic Models".into(),
            authors: vec!["Ho J".into()],
            year: Some(2020),
            venue: "NeurIPS".into(),
            doi: "10.48550/arXiv.2006.11239".into(),
            abstract_text: "DDPM，高质量图像生成的扩散模型。".into(),
            keywords: vec!["diffusion".into(), "image generation".into()],
            tags: vec!["待读".into()],
            status: "未读".into(),
            rating: None,
            pdf_path: String::new(),
            note_path: String::new(),
            created_at: now,
            updated_at: now,
        },
    ];

    let mut conn = db::open(vault)?;
    let tx = conn.transaction()?;

    for p in &examples {
        tx.execute(
            "INSERT OR REPLACE INTO papers
             (id, title, authors, year, venue, doi, abstract_text, keywords, tags, status, rating, pdf_path, note_path, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                p.id, p.title,
                serde_json::to_string(&p.authors)?,
                p.year, p.venue, p.doi, p.abstract_text,
                serde_json::to_string(&p.keywords)?,
                serde_json::to_string(&p.tags)?,
                p.status, p.rating, p.pdf_path, p.note_path,
                p.created_at, p.updated_at,
            ],
        )?;
    }

    // 写默认笔记
    for p in &examples {
        let note_path = vault
            .join(crate::vault::NOTES_DIR)
            .join(crate::vault::NOTES_PAPERS_DIR)
            .join(format!("{}-{}.md", p.id, crate::vault::slug_from_title(&p.title)));
        if !note_path.exists() {
            let body = markdown::default_template(p);
            let fm: serde_yaml::Value = serde_yaml::to_value(p).unwrap_or(serde_yaml::Value::Null);
            markdown::write_note(&note_path, &fm, &body)?;
        }
    }

    // 集合
    let collections = [
        ("col-transformers", "Transformer 系列", 0i64),
        ("col-denoising", "扩散模型", 0i64),
    ];
    for (id, name, _ts) in collections {
        tx.execute(
            "INSERT OR IGNORE INTO collections (id, name, created_at) VALUES (?1, ?2, ?3)",
            params![id, name, now],
        )?;
    }
    tx.execute(
        "INSERT OR IGNORE INTO paper_collections (paper_id, collection_id) VALUES (?1, ?2)",
        params!["demo001attention", "col-transformers"],
    )?;
    tx.execute(
        "INSERT OR IGNORE INTO paper_collections (paper_id, collection_id) VALUES (?1, ?2)",
        params!["demo002bert", "col-transformers"],
    )?;
    tx.execute(
        "INSERT OR IGNORE INTO paper_collections (paper_id, collection_id) VALUES (?1, ?2)",
        params!["demo005diffusion", "col-denoising"],
    )?;

    tx.commit()?;
    Ok(examples.iter().map(|p| p.id.clone()).collect())
}
