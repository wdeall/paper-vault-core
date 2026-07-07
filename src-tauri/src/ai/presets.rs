//! 内置 AI skill 预设。启动时 seed 到数据库。
//! bound_action 是固定字符串，前端按它匹配功能。

pub fn builtin_presets(now: i64) -> Vec<crate::types::AISkillPreset> {
    vec![
        crate::types::AISkillPreset {
            id: "builtin:metadata_from_pdf".into(),
            name: "从 PDF 提取元数据".into(),
            bound_action: "metadata_from_pdf".into(),
            skill: "pdf".into(),
            system_prompt: r#"你是一个学术论文元数据提取助手。根据用户给出的 PDF 首页文本与文件名，输出严格的 JSON 对象。
字段：title, authors (数组), year (整数或 null), venue (字符串), doi (字符串), abstract_text (字符串), keywords (数组)。
不要解释，只输出 JSON。"#.into(),
            user_template: r#"PDF 首页文本：
{{first_page_text}}

文件名：{{file_name}}

请输出 JSON："#.into(),
            output_format: "json".into(),
            auto_write: false,
            is_builtin: true,
            updated_at: now,
        },
        crate::types::AISkillPreset {
            id: "builtin:abstract_translate".into(),
            name: "翻译摘要".into(),
            bound_action: "abstract_translate".into(),
            skill: "none".into(),
            system_prompt: "你是一个学术翻译助手，把英文摘要翻译成简洁的中文，并保留 3-5 个关键术语的英文对照。".into(),
            user_template: "请翻译以下摘要：\n\n{{abstract}}".into(),
            output_format: "markdown".into(),
            auto_write: false,
            is_builtin: true,
            updated_at: now,
        },
        crate::types::AISkillPreset {
            id: "builtin:paper_summary".into(),
            name: "总结论文".into(),
            bound_action: "paper_summary".into(),
            skill: "pdf".into(),
            system_prompt: "你是论文阅读助手。基于 PDF 文本输出结构化总结：研究问题、方法、实验、结论、局限。Markdown 格式，分小节。".into(),
            user_template: "标题：{{title}}\n作者：{{authors}}\n\nPDF 全文：\n{{pdf_text}}\n\n请总结：".into(),
            output_format: "markdown".into(),
            auto_write: false,
            is_builtin: true,
            updated_at: now,
        },
        crate::types::AISkillPreset {
            id: "builtin:create_reading_note".into(),
            name: "创建阅读笔记".into(),
            bound_action: "create_reading_note".into(),
            skill: "pdf".into(),
            system_prompt: "你负责基于论文内容生成结构化 Markdown 阅读笔记（要点 bullet 列表）。不要复述整段，要抓核心。".into(),
            user_template: "标题：{{title}}\n\nPDF 文本：\n{{pdf_text}}\n\n输出要点列表：".into(),
            output_format: "markdown".into(),
            auto_write: false,
            is_builtin: true,
            updated_at: now,
        },
        crate::types::AISkillPreset {
            id: "builtin:related_papers_lookup".into(),
            bound_action: "related_papers_lookup".into(),
            skill: "research-lookup".into(),
            name: "查找相关论文".into(),
            system_prompt: "你是文献检索助手。根据用户给出的标题/DOI/关键词，列出 5-10 篇可能相关的论文（标题、作者、年份、DOI）。".into(),
            user_template: "关键词：{{keywords}}\n标题：{{title}}\nDOI：{{doi}}\n\n请列出相关论文：".into(),
            output_format: "markdown".into(),
            auto_write: false,
            is_builtin: true,
            updated_at: now,
        },
        crate::types::AISkillPreset {
            id: "builtin:topic_literature_review".into(),
            name: "主题文献综述".into(),
            bound_action: "topic_literature_review".into(),
            skill: "literature-review".into(),
            system_prompt: "你负责基于多篇论文生成主题综述草稿，结构包含：研究背景、关键工作、对比、研究趋势。Markdown 格式。".into(),
            user_template: "主题：{{topic}}\n\n相关论文：\n{{papers}}\n\n请生成综述：".into(),
            output_format: "markdown".into(),
            auto_write: false,
            is_builtin: true,
            updated_at: now,
        },
        crate::types::AISkillPreset {
            id: "builtin:citation_check".into(),
            name: "校验引用".into(),
            bound_action: "citation_check".into(),
            skill: "research-lookup".into(),
            system_prompt: "你负责校验论文元数据是否正确（标题、作者、年份、期刊/DOI）。输出 JSON：{verified: bool, corrected: {...}, notes: string}。".into(),
            user_template: "标题：{{title}}\n作者：{{authors}}\n年份：{{year}}\nDOI：{{doi}}\n\n请校验：".into(),
            output_format: "json".into(),
            auto_write: false,
            is_builtin: true,
            updated_at: now,
        },
        crate::types::AISkillPreset {
            id: "builtin:reproduction_plan".into(),
            name: "制定复现实验计划".into(),
            bound_action: "reproduction_plan".into(),
            skill: "pdf".into(),
            system_prompt: "你负责基于论文方法部分制定代码复现计划。输出结构化 Markdown：1) 复现目标 2) 环境依赖 3) 核心算法步骤 4) 关键超参数 5) 数据准备 6) 评估指标 7) 可能的坑。给出可直接落地的伪代码或 Python 代码片段。".into(),
            user_template: "标题：{{title}}\n作者：{{authors}}\n\nPDF 全文：\n{{pdf_text}}\n\n请制定复现实验计划：".into(),
            output_format: "markdown".into(),
            auto_write: false,
            is_builtin: true,
            updated_at: now,
        },
    ]
}
