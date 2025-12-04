// 配置管理模块
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub dashscope_api_key: String,
    #[serde(default)]
    pub siliconflow_api_key: String,
    /// 是否使用实时流式 ASR（WebSocket 模式）
    #[serde(default = "default_use_realtime_asr")]
    pub use_realtime_asr: bool,
    /// 是否启用 LLM 后处理（去重、润色）
    #[serde(default)]
    pub enable_llm_post_process: bool,
    /// LLM 后处理配置
    #[serde(default)]
    pub llm_config: LlmConfig,
}

/// LLM 后处理配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// API 地址
    #[serde(default = "default_llm_endpoint")]
    pub endpoint: String,
    /// 模型名称
    #[serde(default = "default_llm_model")]
    pub model: String,
    /// API Key
    #[serde(default)]
    pub api_key: String,
    /// System Prompt
    #[serde(default = "default_llm_system_prompt")]
    pub system_prompt: String,
}

fn default_llm_endpoint() -> String {
    "https://open.bigmodel.cn/api/paas/v4/chat/completions".to_string()
}

fn default_llm_model() -> String {
    "glm-4-flash-250414".to_string()
}

fn default_llm_system_prompt() -> String {
    "你是一个语音转写润色助手。请在不改变原意的前提下：1）删除重复或意义相近的句子；2）合并同一主题的内容；3）去除「嗯」「啊」等口头禅；4）保留数字与关键信息；5）相关数字和时间不要使用中文；6）整理成自然的段落。输出纯文本即可。".to_string()
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            endpoint: default_llm_endpoint(),
            model: default_llm_model(),
            api_key: String::new(),
            system_prompt: default_llm_system_prompt(),
        }
    }
}

fn default_use_realtime_asr() -> bool {
    true  // 默认启用实时模式
}

impl AppConfig {
    pub fn new() -> Self {
        Self {
            dashscope_api_key: String::new(),
            siliconflow_api_key: String::new(),
            use_realtime_asr: default_use_realtime_asr(),
            enable_llm_post_process: false,
            llm_config: LlmConfig::default(),
        }
    }

    pub fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("无法获取配置目录"))?;
        let app_dir = config_dir.join("PushToTalk");
        std::fs::create_dir_all(&app_dir)?;
        Ok(app_dir.join("config.json"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        tracing::info!("尝试从以下路径加载配置: {:?}", path);
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let config: AppConfig = serde_json::from_str(&content)?;
            tracing::info!("配置加载成功，API Key 长度: {}", config.dashscope_api_key.len());
            Ok(config)
        } else {
            tracing::warn!("配置文件不存在，返回默认配置");
            Ok(Self::new())
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        let content = serde_json::to_string_pretty(self)?;
        tracing::info!("保存配置到: {:?}", path);
        std::fs::write(&path, content)?;
        tracing::info!("配置保存成功");
        Ok(())
    }
}
