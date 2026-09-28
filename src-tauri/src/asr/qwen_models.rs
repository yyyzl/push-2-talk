//! The UI and backend share one catalogue; model IDs never select a protocol by guesswork.
use crate::config::{AsrConfig, AsrProvider, QwenAsrProfile};
use anyhow::{bail, Result};
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QwenMode {
    Http,
    Realtime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QwenProtocol {
    Audio3,
    Qwen3,
    Message,
}

#[derive(Debug, Deserialize)]
pub struct QwenModel {
    pub id: String,
    pub mode: QwenMode,
    pub protocol: QwenProtocol,
}

pub fn catalogue() -> &'static [QwenModel] {
    static MODELS: OnceLock<Vec<QwenModel>> = OnceLock::new();
    MODELS.get_or_init(|| {
        serde_json::from_str(include_str!("../../../src/shared/qwen-models.json"))
            .expect("checked-in Qwen model catalogue must be valid")
    })
}

pub fn resolve_model(id: &str, mode: QwenMode) -> Result<&'static QwenModel> {
    catalogue()
        .iter()
        .find(|model| model.id == id && model.mode == mode)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "千问模型“{}”不支持当前识别模式，请在语音识别设置中选择可用模型；原配置已保留",
                id
            )
        })
}

pub fn profile_model(profile: QwenAsrProfile, mode: QwenMode) -> &'static QwenModel {
    resolve_model(
        match mode {
            QwenMode::Http => profile.http_model(),
            QwenMode::Realtime => profile.realtime_model(),
        },
        mode,
    )
    .expect("legacy profiles must remain in the catalogue")
}

impl AsrConfig {
    pub fn qwen_model(&self, mode: QwenMode) -> Result<&'static QwenModel> {
        let explicit = match mode {
            QwenMode::Http => self.qwen_models.http.as_deref(),
            QwenMode::Realtime => self.qwen_models.realtime.as_deref(),
        };
        match explicit {
            Some(id) => resolve_model(id, mode),
            None => Ok(profile_model(self.qwen_profile, mode)),
        }
    }

    /// Validate only the paths this service will use. An inactive provider's saved
    /// model must not prevent Doubao/SenseVoice from starting.
    pub fn validate_models(&self, realtime: bool) -> Result<()> {
        let realtime = self.selection.active_provider.realtime_enabled(realtime);
        if self.selection.active_provider == AsrProvider::Qwen {
            self.qwen_model(if realtime {
                QwenMode::Realtime
            } else {
                QwenMode::Http
            })?;
        }
        if self.selection.enable_fallback
            && (self.selection.active_provider == AsrProvider::Qwen
                || self.selection.fallback_provider == Some(AsrProvider::Qwen))
        {
            self.qwen_model(QwenMode::Http)?;
        }
        if self.selection.enable_fallback
            && self.selection.fallback_provider == Some(AsrProvider::DoubaoIme)
        {
            bail!("豆包输入法不支持备用 HTTP 识别，请选择其他备用服务");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalogue_has_unique_ids_and_supports_every_legacy_profile() {
        let ids: std::collections::HashSet<_> = catalogue().iter().map(|m| &m.id).collect();
        assert_eq!(ids.len(), catalogue().len());
        for profile in [
            QwenAsrProfile::QwenAudio3_1,
            QwenAsrProfile::QwenAudio3,
            QwenAsrProfile::Qwen3Legacy,
        ] {
            for mode in [QwenMode::Http, QwenMode::Realtime] {
                assert_eq!(profile_model(profile, mode).mode, mode);
            }
        }
    }
    #[test]
    fn snapshot_and_message_resolve_to_their_actual_protocols() {
        assert_eq!(
            resolve_model("qwen3-asr-flash-2026-02-10", QwenMode::Http)
                .unwrap()
                .protocol,
            QwenProtocol::Qwen3
        );
        assert_eq!(
            resolve_model("qwen-audio-3.1-asr-flash-message", QwenMode::Realtime)
                .unwrap()
                .protocol,
            QwenProtocol::Message
        );
        assert!(resolve_model("qwen-audio-3.1-asr-flash-filetrans", QwenMode::Http).is_err());
        assert!(resolve_model("qwen3-asr-flash", QwenMode::Realtime).is_err());
    }
    #[test]
    fn invalid_inactive_model_does_not_block_other_providers_or_modes() {
        let mut config = AsrConfig::default();
        config.qwen_models.http = Some("unknown-kept-for-repair".into());
        assert!(config.validate_models(true).is_ok());
        config.selection.active_provider = AsrProvider::Qwen;
        assert!(config.validate_models(true).is_ok());
        assert!(config.validate_models(false).is_err());
        config.selection.active_provider = AsrProvider::Doubao;
        config.selection.enable_fallback = true;
        config.selection.fallback_provider = Some(AsrProvider::Qwen);
        assert!(config.validate_models(true).is_err());
    }
}
