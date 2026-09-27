use std::env;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use push_to_talk_lib::asr::{
    AsrLanguageMode, DoubaoImeClient, QwenASRClient, QwenAsrProfile, QwenRealtimeClient,
};
use std::time::{Duration, Instant};

#[derive(Debug)]
struct TestArgs {
    provider: String,
    file: PathBuf,
    profile: QwenAsrProfile,
    realtime: bool,
}

fn parse_args(args: &[String]) -> Result<TestArgs> {
    let mut provider = String::from("qwen");
    let mut file: Option<PathBuf> = None;
    let mut profile = QwenAsrProfile::default();
    let mut realtime = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--asr" => {
                i += 1;
                if i >= args.len() {
                    return Err(anyhow!("missing value for --asr"));
                }
                provider = args[i].to_lowercase();
            }
            "--file" => {
                i += 1;
                if i >= args.len() {
                    return Err(anyhow!("missing value for --file"));
                }
                file = Some(PathBuf::from(&args[i]));
            }
            "--qwen-profile" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or_else(|| anyhow!("missing value for --qwen-profile"))?;
                profile = serde_json::from_value(serde_json::json!(value)).map_err(|_| {
                    anyhow!("--qwen-profile expects qwen_audio_3_1|qwen_audio_3|qwen3_legacy")
                })?;
            }
            "--mode" => {
                i += 1;
                realtime = match args.get(i).map(String::as_str) {
                    Some("http") => false,
                    Some("realtime") => true,
                    _ => return Err(anyhow!("--mode expects http|realtime")),
                };
            }
            "-h" | "--help" => {
                println!(
                    "Usage: cargo run --bin test_api -- --asr <qwen|doubao_ime> --file <wav_path> [--qwen-profile <qwen_audio_3_1|qwen_audio_3|qwen3_legacy>] [--mode <http|realtime>]"
                );
                std::process::exit(0);
            }
            other => {
                return Err(anyhow!("unknown argument: {}", other));
            }
        }
        i += 1;
    }

    let file = file.ok_or_else(|| anyhow!("missing required argument: --file <wav_path>"))?;
    if provider != "qwen" && realtime {
        return Err(anyhow!("--mode realtime is only supported for --asr qwen"));
    }
    Ok(TestArgs {
        provider,
        file,
        profile,
        realtime,
    })
}

async fn run_qwen(file: &PathBuf, profile: QwenAsrProfile, realtime: bool) -> Result<()> {
    let api_key = env::var("DASHSCOPE_API_KEY")
        .map_err(|_| anyhow!("DASHSCOPE_API_KEY is required for --asr qwen"))?;
    if api_key.is_empty() {
        return Err(anyhow!("DASHSCOPE_API_KEY is empty"));
    }

    let started = Instant::now();
    let model = if realtime {
        profile.realtime_model()
    } else {
        profile.http_model()
    };
    let text = if realtime {
        let mut reader = hound::WavReader::open(file)?;
        let spec = reader.spec();
        if spec.channels != 1
            || spec.sample_rate != 16000
            || spec.bits_per_sample != 16
            || spec.sample_format != hound::SampleFormat::Int
        {
            return Err(anyhow!("realtime requires 16 kHz mono PCM16 WAV"));
        }
        let samples = reader
            .samples::<i16>()
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let client = QwenRealtimeClient::new_with_profile_and_correction_pairs(
            api_key,
            Vec::new(),
            Vec::new(),
            AsrLanguageMode::Auto,
            profile,
        );
        let mut session = client.start_session().await?;
        let result = async {
            for chunk in samples.chunks(1600) {
                session.send_audio_chunk(chunk).await?;
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            session.commit_audio().await?;
            session.wait_for_result().await
        }
        .await;
        session.close().await?;
        result?
    } else {
        let audio_data = tokio::fs::read(file).await?;
        QwenASRClient::new_with_profile_and_correction_pairs(
            api_key,
            Vec::new(),
            Vec::new(),
            AsrLanguageMode::Auto,
            profile,
        )
        .transcribe_from_memory(&audio_data)
        .await?
    };
    println!(
        "{}",
        serde_json::json!({"model": model, "elapsed_ms": started.elapsed().as_millis(), "text": text})
    );
    Ok(())
}

async fn run_doubao_ime(file: &PathBuf) -> Result<()> {
    let mut client = DoubaoImeClient::new(reqwest::Client::new());
    let text = client.transcribe_wav(file).await?;

    println!("[doubao_ime] transcription result:");
    println!("{}", text);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .init();

    let TestArgs {
        provider,
        file,
        profile,
        realtime,
    } = parse_args(&env::args().collect::<Vec<_>>())?;
    if !file.exists() {
        return Err(anyhow!("file not found: {}", file.display()));
    }

    match provider.as_str() {
        "qwen" => tokio::time::timeout(Duration::from_secs(45), run_qwen(&file, profile, realtime))
            .await
            .map_err(|_| anyhow!("qwen API verification timed out after 45 seconds"))?,
        "doubao_ime" => run_doubao_ime(&file).await,
        _ => Err(anyhow!(
            "unsupported --asr provider: {} (expected qwen|doubao_ime)",
            provider
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_each_qwen_profile_and_transport() {
        for wire in ["qwen_audio_3_1", "qwen_audio_3", "qwen3_legacy"] {
            for mode in ["http", "realtime"] {
                let args = [
                    "test_api",
                    "--file",
                    "test.wav",
                    "--qwen-profile",
                    wire,
                    "--mode",
                    mode,
                ]
                .map(String::from);
                let parsed = parse_args(&args).unwrap();
                assert_eq!(serde_json::to_value(parsed.profile).unwrap(), wire);
                assert_eq!(parsed.realtime, mode == "realtime");
            }
        }
    }

    #[test]
    fn rejects_invalid_or_missing_profile_and_mode() {
        for suffix in [
            vec!["--qwen-profile", "invalid"],
            vec!["--qwen-profile"],
            vec!["--mode", "invalid"],
            vec!["--mode"],
        ] {
            let args = [vec!["test_api", "--file", "test.wav"], suffix]
                .concat()
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>();
            assert!(parse_args(&args).is_err());
        }
        let defaults = parse_args(&["test_api", "--file", "test.wav"].map(String::from)).unwrap();
        assert_eq!(defaults.profile, QwenAsrProfile::default());
        assert!(!defaults.realtime);
    }
}
