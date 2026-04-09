use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use futures_util::StreamExt;

#[cfg(windows)]
use std::ffi::OsStr;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
#[cfg(windows)]
use windows::core::PCWSTR;
#[cfg(windows)]
use windows::Win32::Storage::FileSystem::{
    MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
};

pub const CACHE_FILENAME: &str = "builtin_hotwords_cache.txt";
pub const HOTWORDS_ENDPOINTS: &[&str] = &[
    "https://gh-proxy.org/https://raw.githubusercontent.com/yyyzl/HotWordsLex/main/output/hotwords_latest.txt",
    "https://hk.gh-proxy.org/https://raw.githubusercontent.com/yyyzl/HotWordsLex/main/output/hotwords_latest.txt",
    "https://cdn.jsdelivr.net/gh/yyyzl/HotWordsLex@main/output/hotwords_latest.txt",
    "https://raw.githubusercontent.com/yyyzl/HotWordsLex/main/output/hotwords_latest.txt",
    "https://cdn.gh-proxy.org/https://raw.githubusercontent.com/yyyzl/HotWordsLex/main/output/hotwords_latest.txt",
    "https://edgeone.gh-proxy.org/https://raw.githubusercontent.com/yyyzl/HotWordsLex/main/output/hotwords_latest.txt",
];
pub const REQUEST_TIMEOUT_SECS: u64 = 6;
pub const MAX_HOTWORDS_BYTES: usize = 2 * 1024 * 1024;
pub const MIN_VALID_LINE_COUNT: usize = 1;

pub fn cache_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir().ok_or_else(|| anyhow::anyhow!("无法获取配置目录"))?;
    let app_dir = config_dir.join("PushToTalkOmni");
    std::fs::create_dir_all(&app_dir)?;
    Ok(app_dir.join(CACHE_FILENAME))
}

pub fn load_builtin_hotwords() -> String {
    let path = match cache_path() {
        Ok(path) => path,
        Err(err) => {
            tracing::warn!("获取内置词库缓存路径失败，返回空词库: {}", err);
            return String::new();
        }
    };
    load_builtin_hotwords_from_path(path)
}

pub(crate) fn load_builtin_hotwords_from_path(path: PathBuf) -> String {
    match std::fs::read_to_string(&path) {
        Ok(content) => {
            if validate_hotwords(&content).is_ok() {
                content
            } else {
                tracing::warn!("缓存词库校验失败，删除缓存并返回空词库");
                if let Err(err) = std::fs::remove_file(&path) {
                    tracing::warn!("删除损坏缓存失败: {}", err);
                }
                String::new()
            }
        }
        Err(_) => String::new(),
    }
}

pub fn validate_hotwords(content: &str) -> Result<()> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        anyhow::bail!("词库为空");
    }

    if trimmed.as_bytes().len() > MAX_HOTWORDS_BYTES {
        anyhow::bail!("词库内容过大");
    }

    let mut valid_line_count = 0usize;
    for line in trimmed.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if !is_valid_hotwords_line(line) {
            anyhow::bail!("词库格式不合法");
        }
        valid_line_count += 1;
    }

    if valid_line_count < MIN_VALID_LINE_COUNT {
        anyhow::bail!("词库有效行数不足");
    }

    Ok(())
}

pub fn save_cache_atomic(content: &str) -> Result<()> {
    validate_hotwords(content)?;
    let path = cache_path()?;
    save_cache_atomic_to_path(&path, content)
}

pub(crate) fn save_cache_atomic_to_path(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let unique_suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let tmp_path = path.with_extension(format!("tmp.{}", unique_suffix));

    {
        let mut tmp_file = std::fs::File::create(&tmp_path)?;
        use std::io::Write;
        tmp_file.write_all(content.as_bytes())?;
        tmp_file.sync_all()?;
    }

    if let Err(err) = replace_file(&tmp_path, path) {
        if let Err(cleanup_err) = std::fs::remove_file(&tmp_path) {
            tracing::warn!("替换缓存失败后清理临时文件失败: {}", cleanup_err);
        }
        return Err(err);
    }

    Ok(())
}

fn replace_file(tmp_path: &Path, target_path: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        let from = to_wide_path(tmp_path.as_os_str());
        let to = to_wide_path(target_path.as_os_str());
        let moved = unsafe {
            MoveFileExW(
                PCWSTR(from.as_ptr()),
                PCWSTR(to.as_ptr()),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        };
        if let Err(err) = moved {
            anyhow::bail!("替换缓存文件失败: {}", err);
        }
        return Ok(());
    }

    #[cfg(not(windows))]
    {
        if target_path.exists() {
            std::fs::remove_file(target_path)?;
        }
        std::fs::rename(tmp_path, target_path)?;
        Ok(())
    }
}

#[cfg(windows)]
fn to_wide_path(value: &OsStr) -> Vec<u16> {
    value
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<u16>>()
}

#[cfg(test)]
pub(crate) fn select_first_valid<'a>(
    responses: Vec<(&'a str, anyhow::Result<String>)>,
) -> Result<(String, &'a str)> {
    for (endpoint, result) in responses {
        match result {
            Ok(content) => {
                if validate_hotwords(&content).is_ok() {
                    return Ok((content, endpoint));
                }
                tracing::warn!("内置词库响应格式校验失败: {}", endpoint);
            }
            Err(err) => {
                tracing::warn!("内置词库响应失败: {} -> {}", endpoint, err);
            }
        }
    }
    anyhow::bail!("所有镜像均不可用")
}

pub async fn fetch_remote_hotwords() -> Result<(String, String)> {
    let client = get_http_client()?;

    let mut last_error = "unknown".to_string();
    for endpoint in HOTWORDS_ENDPOINTS {
        let response = match client.get(*endpoint).send().await {
            Ok(response) => response,
            Err(err) => {
                tracing::warn!("内置词库拉取失败 {}: {}", endpoint, err);
                last_error = err.to_string();
                continue;
            }
        };

        if !response.status().is_success() {
            let status = response.status();
            tracing::warn!("内置词库拉取失败 {}: HTTP {}", endpoint, status);
            last_error = format!("HTTP {}", status);
            continue;
        }

        let text = match read_response_text_with_limit(response).await {
            Ok(text) => text,
            Err(err) => {
                tracing::warn!("内置词库读取响应失败 {}: {}", endpoint, err);
                last_error = err.to_string();
                continue;
            }
        };

        if let Err(err) = validate_hotwords(&text) {
            tracing::warn!("内置词库校验失败 {}: {}", endpoint, err);
            last_error = err.to_string();
            continue;
        }

        tracing::info!("内置词库拉取成功: {}", endpoint);
        return Ok((text, endpoint.to_string()));
    }

    anyhow::bail!(
        "所有 {} 个镜像均不可用, 最后错误: {}",
        HOTWORDS_ENDPOINTS.len(),
        last_error
    )
}

fn get_http_client() -> Result<&'static reqwest::Client> {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

    if let Some(client) = CLIENT.get() {
        return Ok(client);
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .build()?;
    let _ = CLIENT.set(client);
    Ok(CLIENT.get().expect("http client should be initialized"))
}

pub(crate) fn validate_response_content_length(content_length: Option<u64>) -> Result<()> {
    if let Some(content_length) = content_length {
        if content_length > MAX_HOTWORDS_BYTES as u64 {
            anyhow::bail!("词库内容过大");
        }
    }
    Ok(())
}

pub(crate) fn append_chunk_with_limit(buffer: &mut Vec<u8>, chunk: &[u8]) -> Result<()> {
    if buffer.len().saturating_add(chunk.len()) > MAX_HOTWORDS_BYTES {
        anyhow::bail!("词库内容过大");
    }
    buffer.extend_from_slice(chunk);
    Ok(())
}

async fn read_response_text_with_limit(response: reqwest::Response) -> Result<String> {
    validate_response_content_length(response.content_length())?;

    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result?;
        append_chunk_with_limit(&mut bytes, &chunk)?;
    }

    String::from_utf8(bytes).map_err(|err| anyhow::anyhow!("词库内容编码不合法: {}", err))
}

fn is_valid_hotwords_line(line: &str) -> bool {
    let Some(remain) = line.strip_prefix('【') else {
        return false;
    };
    let Some((domain, words_with_end)) = remain.split_once("】:[") else {
        return false;
    };
    if domain.trim().is_empty() || !words_with_end.ends_with(']') {
        return false;
    }

    let words_raw = &words_with_end[..words_with_end.len() - 1];
    let mut has_word = false;
    for word in words_raw.split(',') {
        if word.trim().is_empty() {
            return false;
        }
        has_word = true;
    }

    has_word
}

#[cfg(test)]
mod tests {
    use super::{
        append_chunk_with_limit, cache_path, load_builtin_hotwords_from_path,
        save_cache_atomic_to_path, select_first_valid, validate_hotwords,
        validate_response_content_length, MAX_HOTWORDS_BYTES,
    };

    #[test]
    fn cache_path_should_point_to_push_to_talk_config_dir() {
        let path = cache_path().expect("cache path");
        let path_str = path.to_string_lossy().to_lowercase();
        assert!(path_str.contains("pushtotalk"));
        assert!(path_str.ends_with("builtin_hotwords_cache.txt"));
    }

    #[test]
    fn load_builtin_hotwords_should_return_empty_when_cache_missing() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let result = load_builtin_hotwords_from_path(temp.path().join("missing.txt"));
        assert!(result.is_empty());
    }

    #[test]
    fn load_builtin_hotwords_should_return_empty_when_cache_corrupted() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let corrupt_path = temp.path().join("builtin_hotwords_cache.txt");
        std::fs::write(&corrupt_path, "corrupted garbage content").expect("write corrupt file");

        let result = load_builtin_hotwords_from_path(corrupt_path.clone());

        assert!(result.is_empty());
        assert!(!corrupt_path.exists());
    }

    #[test]
    fn validate_hotwords_should_reject_invalid_content() {
        assert!(validate_hotwords("").is_err());
        assert!(validate_hotwords("hello world").is_err());
    }

    #[test]
    fn save_cache_atomic_should_write_complete_content() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let target = temp.path().join("builtin_hotwords_cache.txt");
        save_cache_atomic_to_path(&target, "【AI】:[GPT,Claude]").expect("save cache");
        let readback = std::fs::read_to_string(&target).expect("read cache");
        assert_eq!(readback, "【AI】:[GPT,Claude]");
    }

    #[test]
    fn select_first_valid_should_follow_endpoint_order() {
        let responses: Vec<(&str, anyhow::Result<String>)> = vec![
            ("ep1", Err(anyhow::anyhow!("timeout"))),
            ("ep2", Ok("".to_string())),
            ("ep3", Ok("【AI】:[GPT]".to_string())),
            ("ep4", Ok("【AI】:[Claude]".to_string())),
        ];

        let (content, endpoint) = select_first_valid(responses).expect("select valid endpoint");
        assert_eq!(endpoint, "ep3");
        assert_eq!(content, "【AI】:[GPT]");
    }

    #[test]
    fn select_first_valid_should_fail_when_all_invalid() {
        let responses: Vec<(&str, anyhow::Result<String>)> = vec![
            ("ep1", Err(anyhow::anyhow!("timeout"))),
            ("ep2", Ok("garbage".to_string())),
        ];

        assert!(select_first_valid(responses).is_err());
    }

    #[test]
    fn validate_response_content_length_should_reject_oversized_hint() {
        let oversized = (MAX_HOTWORDS_BYTES as u64) + 1;
        assert!(validate_response_content_length(Some(oversized)).is_err());
        assert!(validate_response_content_length(Some(MAX_HOTWORDS_BYTES as u64)).is_ok());
        assert!(validate_response_content_length(None).is_ok());
    }

    #[test]
    fn append_chunk_with_limit_should_fail_before_buffer_growth() {
        let mut buffer = vec![b'a'; MAX_HOTWORDS_BYTES - 2];
        let oversized_chunk = vec![b'b'; 3];

        let result = append_chunk_with_limit(&mut buffer, &oversized_chunk);
        assert!(result.is_err());
        assert_eq!(buffer.len(), MAX_HOTWORDS_BYTES - 2);
    }
}
