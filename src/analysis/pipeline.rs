use std::sync::Arc;
use serde_json::Value;
use crate::error::AppError;
use crate::llm::{LlmClient, types::{LlmMessage, LlmRequest, ModelTier}};
use crate::analysis::{
    prompts::*,
    types::*,
};

/// 将对话消息格式化为 LLM 输入文本
fn format_messages(messages: &[DialogMessage]) -> String {
    messages.iter().enumerate().map(|(i, m)| {
        let speaker = match m.speaker {
            Speaker::MySelf => "我",
            Speaker::Partner => "对方",
        };
        format!("[{}] {}: {}", i, speaker, m.text)
    }).collect::<Vec<_>>().join("\n")
}

/// 将背景信息格式化为上下文文本
fn format_background(bg: &Option<Background>) -> String {
    let Some(bg) = bg else { return String::new() };

    let mut parts = vec![];

    if let Some(s) = &bg.self_info {
        let mut info = vec!["【关于我】".to_string()];
        if let Some(name) = &s.name { info.push(format!("称呼：{}", name)); }
        if let Some(age) = s.age { info.push(format!("年龄：{}", age)); }
        if let Some(notes) = &s.notes { info.push(format!("补充：{}", notes)); }
        parts.push(info.join("，"));
    }

    if let Some(p) = &bg.partner_info {
        let mut info = vec!["【关于对方】".to_string()];
        if let Some(name) = &p.name { info.push(format!("称呼：{}", name)); }
        if let Some(age) = p.age { info.push(format!("年龄：{}", age)); }
        if let Some(notes) = &p.notes { info.push(format!("补充：{}", notes)); }
        parts.push(info.join("，"));
    }

    if let Some(rel) = &bg.relationship {
        parts.push(format!("【关系现状】{}", rel));
    }

    if parts.is_empty() {
        String::new()
    } else {
        format!("=== 关系背景 ===\n{}\n\n", parts.join("\n"))
    }
}

/// 修复 JSON 字符串值内的裸换行符、控制字符及未转义引号
fn fix_json_strings(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut result = String::with_capacity(s.len() + 32);
    let mut i = 0;
    let mut in_string = false;
    let mut escaped = false;

    while i < chars.len() {
        let ch = chars[i];

        if escaped {
            result.push(ch);
            escaped = false;
            i += 1;
            continue;
        }

        match ch {
            '\\' if in_string => { result.push(ch); escaped = true; }
            '"' if in_string => {
                // 前瞻：跳过空白，看下一个有效字符
                let mut j = i + 1;
                while j < chars.len() && matches!(chars[j], ' ' | '\t' | '\n' | '\r') {
                    j += 1;
                }
                let next = chars.get(j).copied();
                if matches!(next, Some(',') | Some('}') | Some(']') | Some(':') | None) {
                    // 是字符串的结束引号
                    result.push('"');
                    in_string = false;
                } else {
                    // 字符串内容里的裸引号，转义
                    result.push_str("\\\"");
                }
            }
            '"' => { result.push('"'); in_string = true; }
            '\n' if in_string => result.push_str("\\n"),
            '\r' if in_string => result.push_str("\\r"),
            '\t' if in_string => result.push_str("\\t"),
            c if in_string && (c as u32) < 0x20 => {
                result.push_str(&format!("\\u{:04x}", c as u32));
            }
            _ => result.push(ch),
        }
        i += 1;
    }
    result
}

fn parse_json_response<T: serde::de::DeserializeOwned>(text: &str) -> Result<T, AppError> {
    // 提取 JSON 块（模型有时会在前后加说明文字）
    let text = text.trim();
    let start = text.find('{').unwrap_or(0);
    let end = text.rfind('}').map(|i| i + 1).unwrap_or(text.len());
    let json_str = &text[start..end];

    // 替换中文引号为转义引号（避免破坏 JSON 字符串结构）
    let json_str = json_str
        .replace('\u{201c}', "\\\"")
        .replace('\u{201d}', "\\\"")
        .replace('\u{2018}', "'")
        .replace('\u{2019}', "'");

    // 修复裸换行及字符串内未转义引号
    let json_str = fix_json_strings(&json_str);

    // 临时：打印原始响应便于调试
    tracing::debug!("LLM raw response length: {}", json_str.len());

    serde_json::from_str(&json_str)
        .map_err(|e| {
            tracing::error!("JSON parse error: {}\nRaw JSON:\n{}", e, &json_str);
            AppError::LlmError(format!("解析响应失败: {}", e))
        })
}

pub struct AnalysisPipeline {
    llm: Arc<dyn LlmClient>,
}

impl AnalysisPipeline {
    pub fn new(llm: Arc<dyn LlmClient>) -> Self {
        Self { llm }
    }

    pub async fn run(&self, req: &AnalysisRequest) -> Result<AnalysisReport, AppError> {
        let background = format_background(&req.background);
        let dialog = format_messages(&req.messages);
        let context = format!("{}=== 对话记录 ===\n{}", background, dialog);

        // 前 4 步互相独立，并行执行；第 5 步依赖前 4 步结果，串行
        let (emotion, patterns, risks, needs) = tokio::try_join!(
            self.step_emotion(&context),
            self.step_patterns(&context),
            self.step_risks(&context),
            self.step_needs(&context),
        )?;
        let suggestions = self.step_suggestions(&context, &emotion, &patterns, &risks, &needs).await?;

        Ok(AnalysisReport {
            emotion_trajectory: emotion,
            communication_patterns: patterns,
            risk_flags: risks,
            core_needs: needs,
            suggestions,
        })
    }

    async fn step_emotion(&self, context: &str) -> Result<EmotionTrajectory, AppError> {
        let text = self.llm.complete(LlmRequest {
            model: ModelTier::Smart,
            system: Some(EMOTION_TRAJECTORY_SYSTEM.to_string()),
            messages: vec![LlmMessage::user(context)],
            max_tokens: 2000,
        }).await?;

        let v: Value = parse_json_response(&text)?;
        Ok(EmotionTrajectory {
            segments: serde_json::from_value(v["segments"].clone())
                .map_err(|e| AppError::LlmError(e.to_string()))?,
            turning_points: serde_json::from_value(v["turning_points"].clone())
                .map_err(|e| AppError::LlmError(e.to_string()))?,
            summary: v["summary"].as_str().unwrap_or("").to_string(),
        })
    }

    async fn step_patterns(&self, context: &str) -> Result<CommunicationPatterns, AppError> {
        let text = self.llm.complete(LlmRequest {
            model: ModelTier::Smart,
            system: Some(COMMUNICATION_PATTERNS_SYSTEM.to_string()),
            messages: vec![LlmMessage::user(context)],
            max_tokens: 1500,
        }).await?;

        let v: Value = parse_json_response(&text)?;
        Ok(CommunicationPatterns {
            self_attachment_style: v["self_attachment_style"].as_str().unwrap_or("").to_string(),
            partner_attachment_style: v["partner_attachment_style"].as_str().unwrap_or("").to_string(),
            power_dynamic: v["power_dynamic"].as_str().unwrap_or("").to_string(),
            failure_modes: serde_json::from_value(v["failure_modes"].clone()).unwrap_or_default(),
            summary: v["summary"].as_str().unwrap_or("").to_string(),
        })
    }

    async fn step_risks(&self, context: &str) -> Result<Vec<RiskFlag>, AppError> {
        let text = self.llm.complete(LlmRequest {
            model: ModelTier::Smart,
            system: Some(RISK_FLAGS_SYSTEM.to_string()),
            messages: vec![LlmMessage::user(context)],
            max_tokens: 1500,
        }).await?;

        let v: Value = parse_json_response(&text)?;
        serde_json::from_value(v["flags"].clone())
            .map_err(|e| AppError::LlmError(e.to_string()))
    }

    async fn step_needs(&self, context: &str) -> Result<CoreNeeds, AppError> {
        let text = self.llm.complete(LlmRequest {
            model: ModelTier::Smart,
            system: Some(CORE_NEEDS_SYSTEM.to_string()),
            messages: vec![LlmMessage::user(context)],
            max_tokens: 1000,
        }).await?;

        let v: Value = parse_json_response(&text)?;
        Ok(CoreNeeds {
            self_surface: v["self_surface"].as_str().unwrap_or("").to_string(),
            self_deep: v["self_deep"].as_str().unwrap_or("").to_string(),
            partner_surface: v["partner_surface"].as_str().unwrap_or("").to_string(),
            partner_deep: v["partner_deep"].as_str().unwrap_or("").to_string(),
        })
    }

    async fn step_suggestions(
        &self,
        context: &str,
        emotion: &EmotionTrajectory,
        patterns: &CommunicationPatterns,
        risks: &[RiskFlag],
        needs: &CoreNeeds,
    ) -> Result<Vec<Suggestion>, AppError> {
        let summary = format!(
            "{}\n\n=== 前几步分析摘要 ===\n情绪总结：{}\n沟通模式：{}\n风险数量：{}\n我的深层需求：{}\n对方深层需求：{}",
            context,
            emotion.summary,
            patterns.summary,
            risks.len(),
            needs.self_deep,
            needs.partner_deep,
        );

        let text = self.llm.complete(LlmRequest {
            model: ModelTier::Smart,
            system: Some(SUGGESTIONS_SYSTEM.to_string()),
            messages: vec![LlmMessage::user(&summary)],
            max_tokens: 2000,
        }).await?;

        let v: Value = parse_json_response(&text)?;
        serde_json::from_value(v["suggestions"].clone())
            .map_err(|e| AppError::LlmError(e.to_string()))
    }
}
