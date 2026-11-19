pub mod colorspace;
pub mod commands;
pub mod lapjv;
pub mod mosaic;
pub mod saliency;
pub mod tile_loader;
pub mod types;

use commands::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::load_tiles,
            commands::load_destination,
            commands::preview_saliency,
            commands::generate_sorted,
            commands::generate_mosaic,
            commands::apply_blend,
            commands::save_image,
            commands::save_tile_info,
            commands::get_sample_paths,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Photomosaic Studio");
}

