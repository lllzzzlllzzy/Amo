use serde::{Deserialize, Serialize};

/// 用户提交的分析请求
#[derive(Debug, Deserialize)]
pub struct AnalysisRequest {
    pub background: Option<Background>,
    pub messages: Vec<DialogMessage>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Background {
    pub self_info: Option<PersonInfo>,
    pub partner_info: Option<PersonInfo>,
    pub relationship: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PersonInfo {
    pub name: Option<String>,
    pub age: Option<u32>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DialogMessage {
    pub speaker: Speaker,
    pub text: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Speaker {
    #[serde(rename = "self")]
    MySelf,
    Partner,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EmotionTrajectory {
    pub segments: Vec<EmotionSegment>,
    pub turning_points: Vec<TurningPoint>,
    pub summary: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EmotionSegment {
    pub index: usize,
    pub speaker: String,
    pub emotion: String,
    pub intensity: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TurningPoint {
    pub index: usize,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CommunicationPatterns {
    pub self_attachment_style: String,
    pub partner_attachment_style: String,
    pub power_dynamic: String,
    pub failure_modes: Vec<String>,
    pub summary: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RiskFlag {
    pub flag_type: String,
    pub severity: String,
    pub evidence_indices: Vec<usize>,
    pub evidence_text: String,
    pub explanation: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CoreNeeds {
    pub self_surface: String,
    pub self_deep: String,
    pub partner_surface: String,
    pub partner_deep: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Suggestion {
    pub context: String,
    pub original: Option<String>,
    pub rewrite: String,
    pub rationale: String,
}

