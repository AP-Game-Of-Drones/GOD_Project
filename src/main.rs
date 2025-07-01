use std::time::Duration;

use bevy::{app::App, state::app::AppExtStates};

pub mod frontend;
pub mod utils;

include!(concat!(env!("OUT_DIR"), "/build_constants.rs"));

fn main() {
    // utils::client::web_browser::gui_test::test_gui_channels_main();
    let (_handles,chat,web) = utils::initializer::initialize((PROJECT_DIR.to_string() + "/configs/config.toml").as_str()).unwrap();
    std::thread::sleep(Duration::from_secs(10));
    if !chat.is_empty() {
        let subapp = frontend::chat_gui::main_gui(chat);
    } 
    if !web.is_empty() {
        let subapp = frontend::web_gui::main_gui(web);
    } 
    println!("Hello, world!");
}
