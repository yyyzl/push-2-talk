//! TNL 规则定义
//!
//! 包含扩展名白名单、口语符号映射表

use std::collections::{HashMap, HashSet};

/// 口语符号映射
pub struct SpokenSymbolMap {
    map: HashMap<&'static str, char>,
    /// 需要 trim 空格的符号集合（预计算，O(1) 查找）
    trim_symbols: HashSet<char>,
}

impl SpokenSymbolMap {
    pub fn new() -> Self {
        let map = HashMap::from([
            // 点号
            ("点", '.'),
            ("點", '.'), // 繁体
            // 杠号
            ("杠", '-'),
            ("槓", '-'), // 繁体
            ("横杠", '-'),
            ("横线", '-'),
            ("减号", '-'),
            // 斜杠
            ("斜杠", '/'),
            ("斜线", '/'),
            ("斜槓", '/'), // 繁体
            // 下划线
            ("下划线", '_'),
            ("下劃線", '_'), // 繁体
            // 冒号
            ("冒号", ':'),
            ("冒號", ':'), // 繁体
            // @ 符号（邮箱）
            ("艾特", '@'),
            ("at", '@'),
        ]);

        // 预计算 trim 符号集合：映射后的符号 + 反斜杠
        let mut trim_symbols: HashSet<char> = map.values().copied().collect();
        trim_symbols.insert('\\'); // 额外保留反斜杠（路径分隔符）

        Self { map, trim_symbols }
    }

    /// 尝试映射口语符号
    ///
    /// 返回 Some(符号) 如果匹配成功
    pub fn try_map(&self, text: &str) -> Option<char> {
        self.map.get(text).copied()
    }

    /// 判断符号是否需要去除周围空格
    ///
    /// 包含：映射后的符号（`. - / _ : @`）+ 额外的 `\`
    pub(crate) fn is_trim_symbol(&self, symbol: &str) -> bool {
        // 单字符符号才需要 trim（使用 chars().count() 支持非 ASCII）
        let mut chars = symbol.chars();
        if let Some(ch) = chars.next() {
            // 确保只有一个字符
            if chars.next().is_none() {
                return self.trim_symbols.contains(&ch);
            }
        }
        false
    }

    /// 获取所有映射词
    #[allow(dead_code)]
    pub fn keywords(&self) -> Vec<&'static str> {
        self.map.keys().copied().collect()
    }
}

impl Default for SpokenSymbolMap {
    fn default() -> Self {
        Self::new()
    }
}

/// 扩展名白名单
pub struct ExtensionWhitelist {
    extensions: HashSet<&'static str>,
}

impl ExtensionWhitelist {
    pub fn new() -> Self {
        let extensions: HashSet<&'static str> = [
            // 文档
            "md",
            "txt",
            "doc",
            "docx",
            "pdf",
            "rtf",
            // Web
            "html",
            "htm",
            "css",
            "scss",
            "sass",
            "less",
            // JavaScript/TypeScript
            "js",
            "jsx",
            "ts",
            "tsx",
            "mjs",
            "cjs",
            // 系统编程
            "rs",
            "c",
            "cpp",
            "h",
            "hpp",
            "cc",
            "cxx",
            // 脚本
            "py",
            "rb",
            "pl",
            "sh",
            "bash",
            "zsh",
            "fish",
            "bat",
            "ps1",
            "cmd",
            // JVM
            "java",
            "kt",
            "kts",
            "scala",
            "groovy",
            // 其他语言
            "go",
            "swift",
            "m",
            "mm",
            "php",
            "lua",
            "r",
            "jl",
            // 数据/配置
            "json",
            "yaml",
            "yml",
            "toml",
            "xml",
            "ini",
            "conf",
            "cfg",
            // 前端框架
            "vue",
            "svelte",
            "astro",
            // 数据库
            "sql",
            "graphql",
            "gql",
            // 协议
            "proto",
            "thrift",
            // 杂项
            "lock",
            "env",
            "gitignore",
            "dockerignore",
            "editorconfig",
            "prettierrc",
            "eslintrc",
            "babelrc",
            // 图片（常见引用）
            "png",
            "jpg",
            "jpeg",
            "gif",
            "svg",
            "ico",
            "webp",
            // 日志/数据
            "log",
            "csv",
            "tsv",
        ]
        .into_iter()
        .collect();

        Self { extensions }
    }

    /// 检查扩展名是否在白名单中
    pub fn contains(&self, ext: &str) -> bool {
        self.extensions.contains(ext.to_lowercase().as_str())
    }

    /// 获取所有扩展名
    #[allow(dead_code)]
    pub fn all(&self) -> &HashSet<&'static str> {
        &self.extensions
    }
}

impl Default for ExtensionWhitelist {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spoken_symbol_map() {
        let map = SpokenSymbolMap::new();
        assert_eq!(map.try_map("点"), Some('.'));
        assert_eq!(map.try_map("杠"), Some('-'));
        assert_eq!(map.try_map("斜杠"), Some('/'));
        assert_eq!(map.try_map("下划线"), Some('_'));
        assert_eq!(map.try_map("艾特"), Some('@'));
        assert_eq!(map.try_map("at"), Some('@'));
        assert_eq!(map.try_map("无效"), None);
    }

    #[test]
    fn test_extension_whitelist() {
        let whitelist = ExtensionWhitelist::new();
        assert!(whitelist.contains("md"));
        assert!(whitelist.contains("MD")); // 大小写不敏感
        assert!(whitelist.contains("rs"));
        assert!(whitelist.contains("tsx"));
        assert!(!whitelist.contains("xyz"));
    }
}
