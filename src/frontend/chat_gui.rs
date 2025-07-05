use std::{
    collections::HashMap,
    fs::{self},
    io::Cursor,
    sync::Arc,
};

use bevy::prelude::*;
use bevy_egui::{
    EguiContexts, EguiPlugin,
    egui::{self, ColorImage, TextureHandle},
};

use crossbeam_channel::{Receiver, Sender};
use image::DynamicImage;

use crate::{
    frontend::{ChatCommand, ChatEvent},
    utils::fragmentation_handling::{ChatMessages, Message},
};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};

const CHAT_PATH: &str = "assets/chat/media/";

pub struct ChatGuiPlugin {
    pub channels: GuiControllers,
}

impl Plugin for ChatGuiPlugin {
    fn build(&self, app: &mut App) {
        let (stream, handle) = rodio::OutputStream::try_default().unwrap();
        let rodio_player = Box::new(RodioPlayer {
            stream,
            handle,
            sinks: HashMap::new(),
        });

        app.add_plugins((EguiPlugin {
            enable_multipass_for_primary_context: false,
        },));
        app.insert_resource(GuiControllers::new(self.channels.channels.clone()));
        app.insert_resource(TextureCache::default());
        app.insert_resource(MyRodioHandle(rodio_player));
        app.insert_resource(Attachments::default()); // <- Make sure it's added
        app.add_systems(Startup, setup);
        app.add_systems(Update, ui_system);
    }
}


#[derive(Resource, Clone)]
struct Attachments {
    images: Vec<String>,
    audio: Vec<String>,
}

impl Default for Attachments {
    fn default() -> Self {
        let mut images = Vec::new();
        let mut audio = Vec::new();
        let image_path = std::env::current_exe()
            .expect("Faild to get exe path")
            .parent()
            .unwrap()
            .to_path_buf()
            .parent()
            .unwrap()
            .to_path_buf()
            .parent()
            .unwrap()
            .to_path_buf()
            .join(CHAT_PATH.to_string() + "image/")
            .to_str()
            .unwrap()
            .to_string();
        let audio_path = std::env::current_exe()
            .expect("Faild to get exe path")
            .parent()
            .unwrap()
            .to_path_buf()
            .parent()
            .unwrap()
            .to_path_buf()
            .parent()
            .unwrap()
            .to_path_buf()
            .join(CHAT_PATH.to_string() + "audio/")
            .to_str()
            .unwrap()
            .to_string();
        if let Ok(reader) = fs::read_dir(image_path) {
            for entry_res in reader {
                if let Ok(entry) = entry_res {
                    let file = entry.file_name();
                    if let Some(file_name) = file.to_str() {
                        images.push(file_name.to_string());
                    }
                }
            }
        }
        if let Ok(reader) = fs::read_dir(audio_path) {
            for entry_res in reader {
                if let Ok(entry) = entry_res {
                    let file = entry.file_name();
                    if let Some(file_name) = file.to_str() {
                        audio.push(file_name.to_string());
                    }
                }
            }
        }
        Self { images, audio }
    }
}

struct RodioPlayer {
    stream: OutputStream,
    handle: OutputStreamHandle,
    sinks: HashMap<u64, Sink>, // keyed by message index or ID
}

#[derive(Resource)]
struct MyRodioHandle(pub Box<RodioPlayer>);
unsafe impl Sync for MyRodioHandle {}
unsafe impl Send for MyRodioHandle {}


#[derive(Resource, Default)]
struct TextureCache {
    map: HashMap<u64, TextureHandle>,
}

#[derive(Resource, Clone)]
pub struct GuiChannels {
    receiver: Receiver<ChatEvent>,
    sender: Sender<ChatCommand>,
}

impl GuiChannels {
    pub fn new(receiver: Receiver<ChatEvent>, sender: Sender<ChatCommand>) -> Self {
        Self { receiver, sender }
    }
}

#[derive(Resource)]
pub struct GuiControllers {
    channels: HashMap<u8, GuiChannels>,
}

impl GuiControllers {
    pub fn new(channels: HashMap<u8, GuiChannels>) -> Self {
        Self { channels }
    }
}

#[derive(Resource, Default, Clone)]
struct ServerPanel {
    servers: Vec<ServerInfo>,
}

#[derive(Resource, Clone)]
struct ServerInfo {
    id: u8,
    registered: HashMap<u8, bool>,
    selected: bool,
    contacts: Contacts,
}

impl Default for ServerInfo {
    fn default() -> Self {
        Self {
            id: 0,
            registered: HashMap::new(),
            selected: false,
            contacts: Contacts::default(),
        }
    }
}

#[derive(Resource, Default, Clone)]
struct Contacts {
    clients: HashMap<u8, Vec<(u8, bool)>>,
}

#[derive(Resource, Default, Clone)]
struct ClientViewState {
    selected_server: Option<u8>,
    selected_client: Option<u8>,
    chat_pages: HashMap<(u8,u8), ChatPage>,
    input: HashMap<u8, String>,
    attachments_state: bool,
}

#[derive(Resource, Default)]
struct AppState {
    selected_source_client: Option<u8>,
    client_states: HashMap<u8, ClientViewState>,
}

const SENT: u8 = 0;
const RECV: u8 = 1;
#[derive(Debug, Clone)]
pub struct ChatPage {
    contact_id: u8,
    messages: Vec<(u64, u8, ChatMessages)>,
}


fn setup(mut commands: Commands) {
    commands.insert_resource(AppState::default());
    commands.insert_resource(ServerPanel::default());
    commands.insert_resource(Contacts::default());
    commands.insert_resource(ClientViewState::default());
}

fn ui_system(
    mut egui_ctx: EguiContexts,
    mut app_state: ResMut<AppState>,
    mut servers: ResMut<ServerPanel>,
    mut cache: ResMut<TextureCache>,
    channels: ResMut<GuiControllers>,
    mut rodio_player: ResMut<MyRodioHandle>,
    attachments: Res<Attachments>,
    state: Res<super::MainState>,
) {
    let ctx = egui_ctx.ctx_mut();

    if let super::MainState::Chat = *state {
        // Left: Clients & Contacts
        egui::SidePanel::left("left_panel").show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.heading("Clients");
                for (id, gui_c) in &channels.channels {
                    if !app_state.client_states.contains_key(&id) {
                        app_state
                            .client_states
                            .insert(*id, ClientViewState::default());
                    }
                    if ui.button(format!("Client {}", id)).clicked() {
                        app_state.selected_source_client = Some(*id);
                        app_state.client_states.entry(*id).or_default();
                        let _ = gui_c.sender.send(ChatCommand::GetServersType);
                    }
                }

                if let Some(client_id) = app_state.selected_source_client {
                    let client_state = app_state.client_states.get_mut(&client_id).unwrap();
                    if let Some(server_id) = client_state.selected_server {
                        if let Some(server) = servers.servers.iter().find(|s| {
                            s.id == server_id
                                && s.selected
                                && *s.registered.get(&client_id).unwrap_or(&false)
                        }) {
                            if let Some(contacts) = server.contacts.clients.get(&server.id) {
                                ui.separator();
                                ui.label("Contacts:");
                                for (contact_id, _) in contacts {
                                    if *contact_id != client_id {
                                        let text = format!("Chat with {}", contact_id);
                                        if ui.button(text).clicked() {
                                            // Could update selected chat contact
                                            if client_state.selected_client == Some(*contact_id) {
                                                ui.label(format!("Chating with {}", contact_id));
                                            } else {
                                                client_state.selected_client = Some(*contact_id);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            });
        });

        // Right: Servers
        egui::SidePanel::right("right_panel").show(ctx, |ui| {
            ui.heading("Servers");
            if let Some(client_id) = app_state.selected_source_client {
                let mut selected = app_state
                    .client_states
                    .get(&client_id)
                    .unwrap()
                    .selected_server
                    .unwrap_or(0);
                for server in &mut servers.servers {
                    let label = if server.selected {
                        format!("▶ {}", server.id)
                    } else {
                        format!("{}", server.id)
                    };
                    if selected != server.id {
                        server.selected = false;
                    }
                    if *server.registered.get(&client_id).unwrap_or(&false) {
                        if ui.button(label).clicked() {
                            selected = server.id;
                            server.selected = true;
                            app_state
                                .client_states
                                .get_mut(&client_id)
                                .unwrap()
                                .selected_server = Some(server.id);

                            let _ = channels
                                .channels
                                .get(&client_id)
                                .unwrap()
                                .sender
                                .send(ChatCommand::GetClients(server.id));
                        }
                    } else {
                        if ui.button(format!("{}\nRegister", server.id)).clicked() {
                            let _ = channels
                                .channels
                                .get(&client_id)
                                .unwrap()
                                .sender
                                .send(ChatCommand::RegisterTo(server.id));
                        }
                    }
                }
            } else {
                ui.label("Select a client to view servers.");
            }
        });

        // Center: Chat history
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                if let Some(client_id) = app_state.selected_source_client {
                    if let Some(state) = app_state.client_states.get_mut(&client_id) {
                        if let Some(server_id) = state.selected_server {
                            if let Some(contact) = state.selected_client {
                                if let Some(server) = servers.servers.iter().find(|s| {
                                s.id == server_id
                                    && s.selected
                                    && *s.registered.get(&client_id).unwrap_or(&false)
                                }){
                                    if let Some(chat_page) = state.chat_pages.get_mut(&(contact,server_id)) {
                                        ui.heading(format!("Chat with {}", contact));
                                        for (i, pos, msg) in &chat_page.messages {
                                            let mut position = None;
                                            if *pos == SENT {
                                                position =
                                                    Some(egui::Layout::right_to_left(egui::Align::Min));
                                            }
                                            if *pos == RECV {
                                                position =
                                                    Some(egui::Layout::left_to_right(egui::Align::Min))
                                            }
                                        
                                            ui.with_layout(position.unwrap(), |ui| match msg {
                                                ChatMessages::CHATSTRING(_, _, _, s) => {
                                                    ui.label(s);
                                                }
                                                ChatMessages::CHATIMAGE(_, _, _, img) => {
                                                    let id = img_hash(img);
                                                    let texture =
                                                        handle_incoming_image(img, ctx, &mut cache, id);
                                                    let size = egui::Vec2::new(
                                                        img.width() as f32 / 2.0,
                                                        img.height() as f32 / 2.0,
                                                    );
                                                    ui.add(
                                                        egui::Image::new(&texture).fit_to_exact_size(size),
                                                    );
                                                }
                                                ChatMessages::CHATAUDIO(_, _, _, track) => {
                                                    ui.horizontal(|ui| {
                                                        ui.label("🔊 Audio message");
                                                        if ui.button("▶ Play").clicked() {
                                                            if let Some(track_bytes) = Some(&track.bytes) {
                                                                let cursor =
                                                                    Cursor::new(track_bytes.clone());
                                                            
                                                                if let Ok(source) = Decoder::new(cursor) {
                                                                    let sink = Sink::try_new(
                                                                        &rodio_player.0.handle,
                                                                    )
                                                                    .unwrap();
                                                                    sink.append(source);
                                                                    rodio_player.0.sinks.insert(*i, sink);
                                                                } else {
                                                                    error!("Failed to decode audio");
                                                                }
                                                            }
                                                        }
                                                    
                                                        if ui.button("⏹ Stop").clicked() {
                                                            if let Some(sink) =
                                                                rodio_player.0.sinks.remove(i)
                                                            {
                                                                sink.stop();
                                                            }
                                                        }
                                                    });
                                                }
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else {
                    ui.label("Select a client.");
                }
            });
        });

        // Bottom: Input bar
        egui::TopBottomPanel::bottom("input_panel").show(ctx, |ui| {
            if let Some(client_id) = app_state.selected_source_client {
                if let Some(state) = app_state.client_states.get_mut(&client_id) {
                    if let Some(contact) = state.selected_client {
                        let text = state.input.entry(contact).or_default();
                        let time = chrono::Local::now();
                        let str = time.format("%d%H%M").to_string();
                        let mut int = 0;
                        for (i, c) in str.chars().rev().enumerate() {
                            int = int + ((c as u64 - '0' as u64) * (10_u64.pow(i as u32)) as u64);
                        }
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(text);
                            if ui.button("📎").clicked() {
                                state.attachments_state = true;
                            }
                            if state.attachments_state {
                                egui::Window::new("Attachments")
                                    .default_size(bevy_egui::egui::Vec2::new(400., 300.))
                                    .collapsible(true)
                                    .resizable(true)
                                    .show(ctx, |ui| {
                                        ui.columns(2, |columns| {
                                            if let Some(server_id) = state.selected_server {
                                                egui::ScrollArea::vertical()
                                                    .id_salt("Images")
                                                    .show(&mut columns[0], |ui| {
                                                        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                                                            for img in &attachments.images {
                                                                ui.label(img.to_string());
                                                                if ui.button("Send").clicked() {
                                                                    let file_path =
                                                                        std::env::current_exe()
                                                                            .expect(
                                                                                "Faild to get exe path",
                                                                            )
                                                                            .parent()
                                                                            .unwrap()
                                                                            .to_path_buf()
                                                                            .parent()
                                                                            .unwrap()
                                                                            .to_path_buf()
                                                                            .parent()
                                                                            .unwrap()
                                                                            .to_path_buf()
                                                                            .join(
                                                                                CHAT_PATH.to_string()
                                                                                    + "image/"
                                                                                    + img,
                                                                            )
                                                                            .to_str()
                                                                            .unwrap()
                                                                            .to_string();

                                                                    let image = image::open(file_path)
                                                                        .expect("NOT OPENED");
                                                                    let msg =
                                                                        ChatMessages::new_image_msg(
                                                                            client_id,
                                                                            server_id,
                                                                            contact,
                                                                            image.clone(),
                                                                        );
                                                                    let _ = channels
                                                                        .channels
                                                                        .get(&client_id)
                                                                        .unwrap()
                                                                        .sender
                                                                        .send(
                                                                            ChatCommand::SendMessage(
                                                                                server_id,
                                                                                Message::ChatMessages(
                                                                                    msg.clone(),
                                                                                ),
                                                                            ),
                                                                        );
                                                                    state.attachments_state = false;
                                                                    let entry = state
                                                                        .chat_pages
                                                                        .entry((contact,server_id))
                                                                        .or_insert_with(|| ChatPage {
                                                                            contact_id: contact,
                                                                            messages: Vec::new(),
                                                                        });
                                                                    entry
                                                                        .messages
                                                                        .push((int, SENT, msg));
                                                                }
                                                            }
                                                        });
                                                    });
                                                egui::ScrollArea::vertical()
                                                    .id_salt("Audios")
                                                    .show(&mut columns[1], |ui| {
                                                        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                                                            for track in &attachments.audio {
                                                                ui.label(track.to_string());
                                                                if ui.button("Send").clicked() {
                                                                    let file_path =
                                                                        std::env::current_exe()
                                                                            .expect(
                                                                                "Faild to get exe path",
                                                                            )
                                                                            .parent()
                                                                            .unwrap()
                                                                            .to_path_buf()
                                                                            .parent()
                                                                            .unwrap()
                                                                            .to_path_buf()
                                                                            .parent()
                                                                            .unwrap()
                                                                            .to_path_buf()
                                                                            .join(
                                                                                CHAT_PATH.to_string()
                                                                                    + "audio/"
                                                                                    + track,
                                                                            )
                                                                            .to_str()
                                                                            .unwrap()
                                                                            .to_string();
                                                                    let data = fs::read(file_path)
                                                                        .expect("NOT OPENED");
                                                                    let audio = AudioSource {
                                                                        bytes: Arc::from(data),
                                                                    };
                                                                    let msg =
                                                                        ChatMessages::new_audio_msg(
                                                                            client_id,
                                                                            server_id,
                                                                            contact,
                                                                            audio.clone(),
                                                                        );
                                                                    let _ = channels
                                                                        .channels
                                                                        .get(&client_id)
                                                                        .unwrap()
                                                                        .sender
                                                                        .send(
                                                                            ChatCommand::SendMessage(
                                                                                server_id,
                                                                                Message::ChatMessages(
                                                                                    msg.clone(),
                                                                                ),
                                                                            ),
                                                                        );
                                                                    state.attachments_state = false;
                                                                    let entry = state
                                                                        .chat_pages
                                                                        .entry((contact,server_id))
                                                                        .or_insert_with(|| ChatPage {
                                                                            contact_id: contact,
                                                                            messages: Vec::new(),
                                                                        });
                                                                    entry
                                                                        .messages
                                                                        .push((int, SENT, msg));
                                                                }
                                                            }
                                                        });
                                                    });
                                            }
                                        });
                                    });
                            }

                            if ui.button("Send").clicked() {
                                if let Some(server_id) = state.selected_server {
                                    if !text.is_empty() {
                                        let msg = ChatMessages::new_string_msg(
                                            client_id,
                                            server_id,
                                            contact,
                                            text.clone(),
                                        );
                                        let _ =
                                            channels.channels.get(&client_id).unwrap().sender.send(
                                                ChatCommand::SendMessage(
                                                    server_id,
                                                    Message::ChatMessages(msg.clone()),
                                                ),
                                            );
                                        let entry =
                                            state.chat_pages.entry((contact,server_id)).or_insert_with(|| {
                                                ChatPage {
                                                    contact_id: contact,
                                                    messages: Vec::new(),
                                                }
                                            });
                                        entry.messages.push((int, SENT, msg));
                                        text.clear();
                                    }
                                }
                            }
                        });
                    }
                }
            }
        });
    };
    // Handle incoming events
    if let Some(cli) = app_state.selected_source_client {
        while let Ok(event) = channels.channels.get(&cli).unwrap().receiver.try_recv() {
            match event {
                ChatEvent::Servers(id) => {
                    if !servers.servers.iter().any(|s| s.id == id) {
                        servers.servers.push(ServerInfo {
                            id,
                            registered: HashMap::new(),
                            selected: false,
                            contacts: Contacts::default(),
                        });
                    }
                }
                ChatEvent::Registered(id) => {
                    for server in servers.servers.iter_mut() {
                        if server.id == id {
                            server.registered.insert(cli, true);
                        }
                    }
                }
                ChatEvent::Clients(ids) => {
                    if let Some(server) = servers.servers.iter_mut().find(|s| s.selected) {
                        server
                            .contacts
                            .clients
                            .insert(server.id, ids.into_iter().map(|i| (i, false)).collect());
                    }
                }
                ChatEvent::NewMessage(msg) => {
                    let time = chrono::Local::now();
                    let str = time.format("%d%H%M").to_string();
                    let mut int = 0;
                    for (i, c) in str.chars().rev().enumerate() {
                        int = int + ((c as u64 - '0' as u64) * (10_u64.pow(i as u32)) as u64);
                    }
                    match msg {
                        ChatMessages::CHATSTRING(src, srv, target, _)
                        | ChatMessages::CHATIMAGE(src, srv, target, _)
                        | ChatMessages::CHATAUDIO(src, srv, target, _) => {
                            if cli == target {
                                let entry = app_state
                                    .client_states
                                    .get_mut(&cli)
                                    .unwrap()
                                    .chat_pages
                                    .entry((src,srv))
                                    .or_insert_with(|| ChatPage {
                                        contact_id: target,
                                        messages: Vec::new(),
                                    });
                                entry.messages.push((int, RECV, msg));
                            }
                        }
                    }
                }
            }
        }
    }
}

fn handle_incoming_image(
    img: &DynamicImage,
    ctx: &egui::Context,
    cache: &mut TextureCache,
    id: u64,
) -> TextureHandle {
    if let Some(handle) = cache.map.get(&id) {
        return handle.clone();
    }

    let rgba = img.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    let pixels = rgba.as_flat_samples().samples;
    let color_image = ColorImage::from_rgba_unmultiplied(size, pixels);
    let texture = ctx.load_texture(format!("chat_img_{}", id), color_image, Default::default());
    cache.map.insert(id, texture.clone());
    texture
}

fn img_hash(img: &DynamicImage) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    img.clone().into_bytes().hash(&mut hasher);
    hasher.finish()
}
