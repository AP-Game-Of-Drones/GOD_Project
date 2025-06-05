use std::time::Duration;

pub mod frontend;
pub mod utils;

fn main() {
    utils::client::web_browser::gui_test::test_gui_channels_main();
    // let (_handles,channels) = utils::initializer::initialize("./configs/config.toml").unwrap();
    // std::thread::sleep(Duration::from_secs(10));
    // frontend::chat_gui::main_gui(channels);
    println!("Hello, world!");
}
