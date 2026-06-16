//! 示例数据：5 篇占位论文，演示模式使用。
//!
//! v2.0 P0：作者 / 关键词走新结构化表（`paper::insert`）。

use crate::markdown;
use crate::services;
use crate::types::{Paper, PaperStatus};
use std::path::Path;

pub fn load(vault: &Path) -> anyhow::Result<Vec<String>> {
    let now = chrono::Local::now().timestamp_millis();
    let examples: Vec<Paper> = vec![
        Paper {
            id: "demo001attention".into(),
            title: "Attention Is All You Need".into(),
            authors: vec!["Vaswani A".into(), "Shazeer N".into()],
            year: Some(2017),
            venue: "NeurIPS".into(),
            doi: "10.48550/arXiv.1706.03762".into(),
            abstract_text: "提出 Transformer 架构，用自注意力替代 RNN/CNN。".into(),
            keywords: vec!["transformer".into(), "self-attention".into()],
            status: PaperStatus::Read,
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
            status: PaperStatus::Reading,
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
            status: PaperStatus::Unread,
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
            status: PaperStatus::Reading,
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
            status: PaperStatus::Unread,
            rating: None,
            pdf_path: String::new(),
            note_path: String::new(),
            created_at: now,
            updated_at: now,
        },
    ];

    let mut conn = crate::db::open(vault)?;
    // 按 id 去重（已存在就跳过整张 paper，保持幂等）。
    let existing: std::collections::HashSet<String> = {
        let mut stmt = conn.prepare("SELECT id FROM papers")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        rows.filter_map(|r| r.ok()).collect()
    };
    let mut inserted: Vec<String> = Vec::new();
    for p in &examples {
        if existing.contains(&p.id) {
            continue;
        }
        // 走新 schema 插入。
        services::paper::insert(vault, p)?;
        inserted.push(p.id.clone());

        // 写默认笔记
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
    let tx = conn.transaction()?;
    let collections = [
        ("col-transformers", "Transformer 系列"),
        ("col-denoising", "扩散模型"),
    ];
    for (id, name) in collections {
        tx.execute(
            "INSERT OR IGNORE INTO collections (id, name, created_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![id, name, now],
        )?;
    }
    for (paper_id, collection_id) in [
        ("demo001attention", "col-transformers"),
        ("demo002bert", "col-transformers"),
        ("demo005diffusion", "col-denoising"),
    ] {
        tx.execute(
            "INSERT OR IGNORE INTO paper_collections (paper_id, collection_id) VALUES (?1, ?2)",
            rusqlite::params![paper_id, collection_id],
        )?;
    }
    tx.commit()?;

    Ok(inserted)
}
