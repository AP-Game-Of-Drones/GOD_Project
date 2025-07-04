// #![windows_subsystem = "windows"]

use std::time::Duration;

use bevy::{app::App, state::app::AppExtStates};

pub mod frontend;
pub mod utils;


fn main() {
    frontend::run_app();
    // println!("Hello, world!");
}
