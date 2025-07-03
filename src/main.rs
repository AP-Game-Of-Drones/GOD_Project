// #![windows_subsystem = "windows"]

use std::time::Duration;

use bevy::{app::App, state::app::AppExtStates};

pub mod frontend;
pub mod utils;

include!(concat!(env!("OUT_DIR"), "/build_constants.rs"));

fn main() {
    // utils::client::web_browser::gui_test::test_gui_channels_main();
    // let (_handles,chat,web) = utils::initializer::initialize(
    //     std::env::current_exe()
    //     .expect("Failed to get exe_dir")
    //     .parent()
    //     .unwrap()
    //     .to_path_buf()
    //     .parent()
    //     .unwrap()
    //     .to_path_buf()
    //     .parent()
    //     .unwrap()
    //     .to_path_buf()
    //     .join("configs/config.toml")
    //     .to_str()
    //     .unwrap())
    //     .unwrap();
    // std::thread::sleep(Duration::from_secs(10));
    // if !chat.is_empty() {
    //     let subapp = frontend::chat_gui::main_gui(chat);
    // }
    // if !web.is_empty() {
    //     let subapp = frontend::web_gui::main_gui(web);
    // }
    frontend::run_prim_with_seconds();
    // println!("Hello, world!");
}
