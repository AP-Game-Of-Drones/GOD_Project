use super::super::controller::*;

#[derive(Event)]
pub struct UpdateNodesEvent;

#[derive(Event)]
pub struct NodeMovedEvent {
    pub node_id: u8,
    pub new_position: Option<Vec3>,
}
#[derive(Event)]
pub struct MakeLinesEvent;

#[derive(Event)]
pub struct SpawnDroneEvent {
    pub id: u8,
}

#[derive(Event)]
pub struct ChangePdrEvent {
    pub entity: Entity,
    pub new_pdr: f32,
}

#[derive(Event)]
pub enum PacketCreateEvent {
    NodeEvent(NodeEvent),
    DroneEvent(DroneEvent),
}

#[derive(Event)]
pub struct PacketAddHopEvent {
    pub drone_event: DroneEvent,
}

#[derive(Component)]
pub struct HopQueue(pub VecDeque<(u8, Option<u8>)>);

#[derive(Component)]
pub struct PacketInfo {
    pub session_id: u64,
    pub hops: Vec<u8>,
    pub last_hop_index: usize,
}

#[derive(PartialEq, Debug)]
pub enum NodeType {
    Drone,
    Client,
    Server,
}

#[derive(Component)]
pub struct ScNode {
    pub id: u8,
    pub connected_node_ids: Vec<u8>,
    pub node_type: NodeType,
    pub pdr: f32,
}

#[derive(Component)]
pub struct DroneText;

#[derive(Component)]
pub struct TextBox;

#[derive(Component)]
pub struct TextboxTopText;

#[derive(Component)]
pub struct TextWarn;

#[derive(Resource, PartialEq, Eq, Clone, Copy)]
pub enum ActiveMode {
    None,
    Add,
    Crash,
    Connect,
    Pdr,
}

#[derive(Resource)]
pub struct SelectedNode(pub Option<Entity>);

#[derive(Component)]
pub struct DroneAdd;

#[derive(Component)]
pub struct ConfirmShown;

#[derive(Component)]
pub struct DroneIdInput;

#[derive(Component)]
pub enum ButtonLabel {
    Add,
    Crash,
    Connect,
    Pdr,
    Done,
}

#[derive(Component)]
pub struct ButtonColors {
    pub normal: Color,
    pub hovered: Color,
    pub pressed: Color,
}

#[derive(Component)]
pub struct DroneSelector {
    pub index: usize,
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum SelectorDirection {
    Left,
    Right,
}

#[derive(Component)]
pub struct PacketMotion {
    pub start: Vec3,
    pub end: Vec3,
    pub progress: f32,
}
