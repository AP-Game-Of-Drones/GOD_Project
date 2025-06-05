use bevy::audio::AudioSource;
use image::DynamicImage;


pub mod chat_gui;
pub mod web_gui;

#[derive(Debug, Clone)]
pub enum ChatCommand {
    RegisterTo(u8),
    GetServersType,
    GetClients(u8),
    SendMessage(u8, super::utils::fragmentation_handling::Message),
}

#[derive(Debug, Clone)]
pub enum ChatEvent {
    Servers(u8),
    Clients(Vec<u8>),
    Registered(u8),
    NewMessage(super::utils::fragmentation_handling::ChatMessages),
}

#[derive(Debug, Clone)]
pub enum WebCommand {
    GetServersType,
    GetAllText(u8),
    GetAllMedia(u8),
    GetText(u8, String),
    GetMedia(u8, String),
}

#[derive(Debug, Clone)]
pub enum WebEvent {
    Servers(u8, u8),
    AllMedia(Vec<String>),
    AllText(Vec<String>),
    Audio(AudioSource),
    Image(DynamicImage),
    Text(Vec<String>),
}
