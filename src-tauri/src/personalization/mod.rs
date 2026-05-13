//! 个性化二次解码 MVP。
//!
//! 这一层消费 ASR 已经输出的文本，不重新识别音频。当前先服务 Week 1
//! mini eval，后续再接入运行时 pipeline。

mod correction_pair_store;
mod engine;
mod phonetic_keys;

pub use correction_pair_store::{
    default_correction_pairs_path, record_accepted_correction_pair, CorrectionPair,
    CorrectionPairStore,
};
pub use engine::{ConversionDiagnostics, ConversionResult, PersonalizationEngine};
