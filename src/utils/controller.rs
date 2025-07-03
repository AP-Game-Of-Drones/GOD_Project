use bevy::{
    asset::RenderAssetUsages,
    color::palettes::basic::*,
    ecs::observer::*,
    math::Vec3,
    prelude::*,
    render::camera::ScalingMode,
    render::mesh::{Mesh, PrimitiveTopology},
    ui::FocusPolicy,
    window::*,
    winit::WakeUp,
    winit::WinitPlugin,
};
use bevy_simple_text_input::*;
use crossbeam_channel::*;
use rand::*;
use std::collections::VecDeque;
use std::{
    collections::HashMap,
    f32::consts::PI,
    fmt::Debug,
    fs,
    thread::{self, JoinHandle},
};
use toml::to_string;
use toml::{self};
use wg_2024::{config::Config, controller::*, drone::Drone, network::*, packet::*};

pub mod components;
pub mod logic;
pub mod setup;
pub mod systems;

use components::*;
use logic::*;
use setup::*;
use systems::*;

pub enum NodeCommand {
    AddSender(NodeId,Sender<Packet>),
    RemoveSender(NodeId),
}
#[derive(Debug)]
pub enum NodeEvent {
    PacketSent(Packet),
    ControllerShortcut(Packet),
}

const DRONE_NAMES: [&str; 10] = [
    "BagelBomber",
    "BetterCallDrone",
    "RustRoveri",
    "GetDroned",
    "CppEnjoyers",
    "D.R.O.N.E",
    "NullPointer",
    "Rustafarian",
    "DrOnes",
    "Rusteze",
];
const UI_HEIGHT: f32 = 10.0;
const WINDOW_WIDTH: f32 = 1920.0;
const WINDOW_HEIGHT: f32 = 1080.0 - 1080.0 / UI_HEIGHT;
const PACKET_SPEED: f32 = 1000.0;

#[derive(Resource)]
pub struct SimulationController {
    sender_drone_command: HashMap<NodeId, Sender<DroneCommand>>, //for sc use (crash, pdr, add sender, remove sender)
    sender_client_server_command: HashMap<NodeId, Sender<NodeCommand>>, //for sc use (add sender, remove sender)
    receiver_drone_event: Receiver<DroneEvent>, //for sc use (animations & shortcuts)
    receiver_client_server_event: Receiver<NodeEvent>, //for sc use (animations)
    sender_drone_event: Sender<DroneEvent>,     //for new drones (they use it to send sc events)
    sender_node_packet: HashMap<NodeId, Sender<Packet>>, // SERVER & CLIENT SENDERS ALSO NEEDED, for sc use (connecting nodes with add sender)
}

impl SimulationController {
    pub fn new(
        sender_drone_command: HashMap<NodeId, Sender<DroneCommand>>,
        sender_client_server_command: HashMap<NodeId, Sender<NodeCommand>>,
        receiver_drone_event: Receiver<DroneEvent>,
        receiver_client_server_event: Receiver<NodeEvent>,
        sender_drone_event: Sender<DroneEvent>,
        sender_node_packet: HashMap<NodeId, Sender<Packet>>,
    ) -> Self {
        Self {
            sender_drone_command,
            sender_client_server_command,
            receiver_drone_event,
            receiver_client_server_event,
            sender_drone_event,
            sender_node_packet,
        }
    }

    pub fn run(self) {
        let mut winit_plugin = WinitPlugin::<WakeUp>::default();
        winit_plugin.run_on_any_thread = true; //TODO?: BEVY IN UN THREAD NON MAIN NON FUNZIONA SU MAC / BEVY IN A NON MAIN THREAD DOESNT WORK ON MAC

        App::new()
            .insert_resource(ClearColor(Color::srgb(0.1, 0.5, 0.9)))
            .add_plugins((
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
                    }),
                TextInputPlugin,
                winit_plugin,
            ))
            .add_event::<UpdateNodesEvent>()
            .add_event::<NodeMovedEvent>()
            .add_event::<MakeLinesEvent>()
            .add_event::<SpawnDroneEvent>()
            .add_event::<ChangePdrEvent>()
            .add_event::<PacketCreateEvent>()
            .add_event::<PacketAddHopEvent>()
            .insert_resource(ActiveMode::None)
            .insert_resource(self)
            .insert_resource(SelectedNode(None))
            .add_systems(Startup, (setup, setup_ui))
            .add_systems(
                Update,
                (
                    crossbeam_listener,
                    packet_spawn,
                    packet_add_hop.after(packet_spawn),
                    packet_move,
                    update_packet_ends,
                    button_system,
                    button_action,
                    update_selector,
                    change_pdr,
                    spawn_drone.before(update_nodes),
                    update_nodes,
                    make_lines.after(update_nodes),
                ),
            )
            .add_observer(crash_target)
            .add_observer(manage_highlight)
            .add_observer(reset_highlight)
            .add_observer(connect_nodes)
            .add_observer(change_pdr_target)
            .run();
    }
}

pub struct SimulationControllerPlugin {}

impl Plugin for SimulationControllerPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(Color::srgb(0.1, 0.5, 0.9)));
        app.add_event::<UpdateNodesEvent>();
        app.add_event::<NodeMovedEvent>();
        app.add_event::<MakeLinesEvent>();
        app.add_event::<SpawnDroneEvent>();
        app.add_event::<ChangePdrEvent>();
        app.add_event::<PacketCreateEvent>();
        app.add_event::<PacketAddHopEvent>();
        app.insert_resource(ActiveMode::None);
        app.insert_resource(SelectedNode(None));
        app.add_systems(Startup, (setup, setup_ui));
        app.add_systems(
            Update,
            (
                crossbeam_listener,
                packet_spawn,
                packet_add_hop.after(packet_spawn),
                packet_move,
                update_packet_ends,
                button_system,
                button_action,
                update_selector,
                change_pdr,
                spawn_drone.before(update_nodes),
                update_nodes,
                make_lines.after(update_nodes),
            ),
        );
        app.add_observer(crash_target);
        app.add_observer(manage_highlight);
        app.add_observer(reset_highlight);
        app.add_observer(connect_nodes);
        app.add_observer(change_pdr_target);
    }
}
