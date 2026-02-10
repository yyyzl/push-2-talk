use std::env;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use base64::{engine::general_purpose, Engine as _};
use push_to_talk_lib::asr::DoubaoImeClient;

fn parse_args() -> Result<(String, PathBuf)> {
    let mut provider = String::from("qwen");
    let mut file: Option<PathBuf> = None;

    let args: Vec<String> = env::args().collect();
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
            "-h" | "--help" => {
                println!(
                    "Usage: cargo run --bin test_api -- --asr <qwen|doubao_ime> --file <wav_path>"
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
    Ok((provider, file))
}

async fn run_qwen(file: &PathBuf) -> Result<()> {
    let api_key = env::var("DASHSCOPE_API_KEY")
        .map_err(|_| anyhow!("DASHSCOPE_API_KEY is required for --asr qwen"))?;
    if api_key.is_empty() {
        return Err(anyhow!("DASHSCOPE_API_KEY is empty"));
    }

    let audio_data = tokio::fs::read(file).await?;
    let audio_base64 = general_purpose::STANDARD.encode(&audio_data);

    let request_body = serde_json::json!({
        "model": "qwen3-asr-flash",
        "input": {
            "messages": [
                {
                    "role": "system",
                    "content": [{"text": ""}]
                },
                {
                    "role": "user",
                    "content": [
                        {
                            "audio": format!("data:audio/wav;base64,{}", audio_base64)
                        }
                    ]
                }
            ]
        },
        "parameters": {
            "result_format": "message",
            "enable_itn": true
        }
    });

    let url =
        "https://dashscope.aliyuncs.com/api/v1/services/aigc/multimodal-generation/generation";
    let client = reqwest::Client::new();
    let response = client
        .post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response.text().await?;
        return Err(anyhow!("qwen api error ({}): {}", status, error_text));
    }

    let result: serde_json::Value = response.json().await?;
    let text = result["output"]["choices"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|choice| choice["message"]["content"].as_array())
        .and_then(|content| content.first())
        .and_then(|item| item["text"].as_str())
        .ok_or_else(|| anyhow!("failed to parse qwen transcription result"))?;

    println!("[qwen] transcription result:");
    println!("{}", text);
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
    tracing_subscriber::fmt::init();

    let (provider, file) = parse_args()?;
    if !file.exists() {
        return Err(anyhow!("file not found: {}", file.display()));
    }

    match provider.as_str() {
        "qwen" => run_qwen(&file).await,
        "doubao_ime" => run_doubao_ime(&file).await,
        _ => Err(anyhow!(
            "unsupported --asr provider: {} (expected qwen|doubao_ime)",
            provider
        )),
    }
}
