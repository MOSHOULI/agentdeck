//! 配置注入适配器模块（M2）。
//!
//! 适配器模式：统一接口 → 各格式 Writer。
//! 支持格式：TOML section / JSON merge / JSONC merge / YAML section / plain text。

pub enum Format {
    TomlSection,
    JsonMerge,
    JsoncMerge,
    YamlSection,
    PlainText,
}

/// 注入配置到目标文件
pub fn inject(
    _format: Format,
    _target_path: &std::path::Path,
    _content: &str,
) -> crate::Result<()> {
    todo!("M2: 实现配置注入适配器")
}
