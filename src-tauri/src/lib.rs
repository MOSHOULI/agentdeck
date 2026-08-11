/// AgentDeck 桌面应用 —— Tauri 2 后端。
///
/// 注册 Tauri commands，加载 aisync-core 引擎。
/// M3：启动时挂定时采集线程（每小时），状态存 CollectionState。
mod commands;

/// 运行 Tauri 应用
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(commands::memory::CollectionState::default())
        .invoke_handler(tauri::generate_handler![
            commands::health_check,
            commands::deploy::get_agents,
            commands::deploy::install_agent,
            commands::deploy::uninstall_agent,
            commands::deploy::verify_agent_auth,
            commands::memory::collect_memory,
            commands::memory::search_memory,
            commands::memory::inject_memory,
            commands::memory::toggle_auto_inject,
            commands::memory::recent_memories,
            commands::memory::memory_collection_status,
            commands::vault::save_api_key,
            commands::vault::list_api_keys,
            commands::vault::get_api_key,
            commands::vault::delete_api_key,
            commands::vault::import_v2_keys,
            commands::backup::list_backups,
            commands::backup::restore_backup,
            commands::sync::get_mcp_servers,
            commands::sync::toggle_mcp_for_agent,
            commands::update::check_updates,
            commands::update::apply_updates,
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            std::thread::spawn(move || commands::memory::scheduled_collect_loop(handle));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("启动 AgentDeck 失败");
}
