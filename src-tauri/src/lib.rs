/// AgentDeck 桌面应用 —— Tauri 2 后端。
///
/// 注册 Tauri commands，加载 aisync-core 引擎。
/// M0 骨架：提供健康检查命令，后续 M1-M6 逐步填充。

mod commands;

/// 运行 Tauri 应用
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::health_check,
        ])
        .run(tauri::generate_context!())
        .expect("启动 AgentDeck 失败");
}
