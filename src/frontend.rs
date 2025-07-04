use std::{
    io::Write,
    ops::DerefMut,
    sync::{Arc, Mutex},
};

use bevy::{
    app::{AppExit, InternedAppLabel, Startup, SubApp, SubApps, Update}, asset::AssetPlugin, audio::AudioSource, ecs::{
        component::Component,
        event::{EventReader, EventWriter},
        resource::Resource,
        system::{Commands, Res, ResMut},
        world::World,
    }, input::keyboard::{Key, KeyCode, KeyboardInput}, prelude::{default, PluginGroup}, state::app::AppExtStates, ui::widget::Label, window::{Window, WindowMode, WindowPlugin, WindowResolution}, winit::{WakeUp, WinitPlugin}, DefaultPlugins
};
use bevy_egui::{
    EguiContexts, EguiPlugin,
    egui::{FontId, RichText},
};
use bevy_simple_text_input::TextInputPlugin;
use image::DynamicImage;
use rodio::InputDevices;
use wg_2024::config::Config;

use crate::{
    frontend::{
        self,
        chat_gui::{ChatGuiPlugin, GuiControllers},
        web_gui::WebGuiPlugin,
    },
    utils::{self, controller},
};

use bevy::app::AppLabel;

#[derive(Debug, Hash, PartialEq, Eq, Clone, AppLabel)]
struct GuiApp;

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
    ErrNoAllMedia,
    ErrNoAllText,
    ErrMediaNotFound,
    ErrTextNotFound
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Resource)]
pub enum MainState {
    Start,
    Chat,
    Web,
    Sim,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Resource)]
enum AppType {
    Chat,
    Web,
}

#[derive(Resource)]
pub struct Configs(pub Config);

pub fn run_app() {
    let (config_path, the_one) = utils::initializer::choose_config_cli();

    let (_handles, chat, web, simulation_controller, configs) =
        utils::initializer::initialize(config_path.to_str().unwrap(),the_one).unwrap();

    let mut app = bevy::app::App::new();
    let mut winit_plugin = WinitPlugin::<WakeUp>::default();

    let exe_path = std::env::current_exe().expect("Failed to get current exe path");
    let exe_dir = exe_path
        .parent()
        .unwrap()
        .to_path_buf()
        .parent()
        .unwrap()
        .to_path_buf()
        .parent()
        .unwrap()
        .to_path_buf();
    let asset_path = exe_dir.join("assets");
    winit_plugin.run_on_any_thread = false; //TODO?: BEVY IN UN THREAD NON MAIN NON FUNZIONA SU MAC / BEVY IN A NON MAIN THREAD DOESNT WORK ON MAC
    app.add_plugins((
        DefaultPlugins
            .build()
            .disable::<bevy::winit::WinitPlugin>()
            .set(WindowPlugin {
                primary_window: Some(Window {
                    resolution: WindowResolution::new(1600., 900.),
                    mode: WindowMode::Windowed,
                    title: "Simulation Controller".to_string(),
                    //resizable: false,
                    ..default()
                }),
                ..default()
            })
            .set(AssetPlugin {
                file_path: asset_path.to_string_lossy().to_string(),
                ..default()
            }),
        TextInputPlugin,
        winit_plugin,
    ));

    if !chat.is_empty() {
        app.add_plugins(ChatGuiPlugin {
            channels: GuiControllers::new(chat),
        });
        app.insert_resource(AppType::Chat);
    }
    if !web.is_empty() {
        app.add_plugins(WebGuiPlugin {
            channels: web_gui::GuiControllers::new(web),
        });
        app.insert_resource(AppType::Web);
    }
    app.insert_resource(Configs(configs));
    app.insert_resource(simulation_controller);
    app.add_systems(Startup, setup);
    app.add_systems(Update, upds);
    app.insert_resource(MainState::Sim);
    app.add_plugins(super::utils::controller::SimulationControllerPlugin {});
    app.run();
}

fn setup(mut c: Commands) {
}

fn upds(
    mut state: ResMut<MainState>,
    apptype: Res<AppType>,
    mut keys:  EventReader<KeyboardInput>
) {
    for event in keys.read() {
        if event.key_code == KeyCode::ArrowLeft && *state != MainState::Sim {
            *state = MainState::Sim;
        }
        if let AppType::Chat = *apptype {
            if event.key_code == KeyCode::ArrowRight && *state != MainState::Chat {
                *state = MainState::Chat;
            }
        }
        if let AppType::Web = *apptype {
            if event.key_code == KeyCode::ArrowRight && *state != MainState::Web {
                *state = MainState::Web;
            }
        }
    }
}

// TODO : Made to choose config at runtime, problem: refractor on setup of gui and initialize fn
// fn ui_select_value(
//     mut shared: ResMut<StartConfig>,
//     mut ectx: EguiContexts,
// ) {
//     let ctx = ectx.ctx_mut();
//     bevy_egui::egui::CentralPanel::default().show(ctx, |ui|{
//         let config_path = std::env::current_exe()
//         .unwrap()
//         .parent()
//         .unwrap()
//         .to_path_buf()
//         .parent()
//         .unwrap()
//         .to_path_buf()
//         .parent()
//         .unwrap()
//         .to_path_buf()
//         .join("configs/");

//         let reader = std::fs::read_dir(config_path).expect("No config dir found");
//         ui.vertical(|ui|{
//             for entry in reader {
//                 if let Ok(file) = entry{
//                     let config = file.path().to_str().unwrap().to_string();
//                     ui.label(RichText::new(config.clone()).font(FontId::proportional(10.)).color(bevy_egui::egui::Color32::RED));
//                     if ui.button("->").clicked() {
//                         *shared = StartConfig(Some(config.clone()));
//                     };
//                 }
//                 ui.separator();
//             }

//         })

//     });
// }
