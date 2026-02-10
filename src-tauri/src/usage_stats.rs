// src-tauri/src/usage_stats.rs

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 使用统计数据（前端使用 camelCase）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageStats {
    /// 总录音时长（毫秒）
    #[serde(default)]
    pub total_recording_ms: u64,
    /// 总录音条数
    #[serde(default)]
    pub total_recording_count: u64,
    /// 总识别字数
    #[serde(default)]
    pub total_recognized_chars: u64,
}

impl Default for UsageStats {
    fn default() -> Self {
        Self {
            total_recording_ms: 0,
            total_recording_count: 0,
            total_recognized_chars: 0,
        }
    }
}

impl UsageStats {
    /// 获取统计数据文件路径
    pub fn stats_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir().ok_or_else(|| anyhow::anyhow!("无法获取配置目录"))?;
        let app_dir = config_dir.join("PushToTalk");
        std::fs::create_dir_all(&app_dir)?;
        Ok(app_dir.join("stats.json"))
    }

    /// 从文件加载统计数据
    pub fn load() -> Result<Self> {
        let path = Self::stats_path()?;
        tracing::info!("尝试从以下路径加载统计数据: {:?}", path);

        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let stats: UsageStats = serde_json::from_str(&content)?;
            tracing::info!("统计数据加载成功: {:?}", stats);
            Ok(stats)
        } else {
            tracing::warn!("统计数据文件不存在，返回默认值");
            Ok(Self::default())
        }
    }

    /// 保存统计数据到文件
    pub fn save(&self) -> Result<()> {
        let path = Self::stats_path()?;
        let content = serde_json::to_string_pretty(self)?;
        tracing::debug!("保存统计数据到: {:?}", path);
        std::fs::write(&path, content)?;
        tracing::debug!("统计数据保存成功");
        Ok(())
    }

    /// 更新统计数据并自动保存
    ///
    /// # 参数
    /// - `recording_ms`: 本次录音时长（毫秒）
    /// - `recognized_chars`: 本次识别字数
    ///
    /// # 错误处理
    /// 如果保存失败，会回滚内存中的更新以保持一致性
    pub fn update_and_save(&mut self, recording_ms: u64, recognized_chars: u64) -> Result<()> {
        // 保存旧值用于回滚
        let old_ms = self.total_recording_ms;
        let old_count = self.total_recording_count;
        let old_chars = self.total_recognized_chars;

        // 更新内存中的数据
        self.total_recording_ms += recording_ms;
        self.total_recording_count += 1;
        self.total_recognized_chars += recognized_chars;

        tracing::info!(
            "统计数据已更新: 录音时长 +{}ms, 识别字数 +{}, 总计: {}min / {} 条 / {} 字",
            recording_ms,
            recognized_chars,
            self.total_recording_ms / 60000,
            self.total_recording_count,
            self.total_recognized_chars
        );

        // 尝试保存到文件
        if let Err(e) = self.save() {
            // 保存失败，回滚内存更新以保持一致性
            self.total_recording_ms = old_ms;
            self.total_recording_count = old_count;
            self.total_recognized_chars = old_chars;
            tracing::error!("保存统计数据失败，已回滚内存更新: {}", e);
            return Err(e);
        }

        Ok(())
    }
}
