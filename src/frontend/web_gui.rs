use std::{collections::HashMap, io::Cursor};

use bevy::prelude::*;
use bevy_egui::{
    EguiContexts, EguiPlugin,
    egui::{self, Color32, ColorImage, RichText, TextureHandle},
};
use crossbeam_channel::{Receiver, Sender};
use image::DynamicImage;

use crate::{
    frontend::{MainState, WebCommand, WebEvent},
    utils::fragmentation_handling::{ContentResponse, DefaultResponse},
};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};

const TEXTSERVER: u8 = 1;
const MEDIASERVER: u8 = 2;

pub struct WebGuiPlugin {
    pub channels: GuiControllers,
}

impl Plugin for WebGuiPlugin {
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
        app.insert_resource(MyRodioHandle(rodio_player)); // <- Make sure it's added
        app.add_systems(Startup, setup);
        app.add_systems(Update, ui_system);
    }
}

struct RodioPlayer {
    stream: OutputStream,
    handle: OutputStreamHandle,
    sinks: HashMap<u64, Sink>, // keyed by message index or ID
}

#[derive(Resource)]
struct MyRodioHandle( Box<RodioPlayer>);
unsafe impl Sync for MyRodioHandle {}
unsafe impl Send for MyRodioHandle {}

#[derive(Resource, Default)]
struct TextureCache {
    map: HashMap<u64, TextureHandle>,
}

#[derive(Resource, Clone)]
pub struct GuiChannels {
    receiver: Receiver<WebEvent>,
    sender: Sender<WebCommand>,
}

impl GuiChannels {
    pub fn new(receiver: Receiver<WebEvent>, sender: Sender<WebCommand>) -> Self {
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
struct WebViewState {
    web_pages: WebPage,
}

#[derive(Resource, Default)]
struct AppState {
    selected_source_client: Option<u8>,
    browsers_states: HashMap<u8, WebViewState>,
    error_state: bool
}

#[derive(Debug, Clone, Default)]
struct WebPage {
    default: Vec<DefaultResponse>,
    content: Vec<ContentResponse>,
}

#[derive(Resource, Default, Clone)]
struct ServerPanel {
    servers: Vec<ServerInfo>,
}

#[derive(Resource, Clone)]
struct ServerInfo {
    id: u8,
    server_type: u8,
}

impl Default for ServerInfo {
    fn default() -> Self {
        Self {
            id: 0,
            server_type: 0,
        }
    }
}

#[derive(Resource, Default)]
struct ImageViewer {
    open: bool,
    current_image_id: Option<u64>,
}

#[derive(Resource, Default)]
struct AudioPlayer {
    open: bool,
    current_audio_id: Option<u64>,
}

fn setup(mut commands: Commands) {
    commands.insert_resource(AppState::default());
    commands.insert_resource(WebViewState::default());
    commands.insert_resource(ServerPanel::default());
    commands.insert_resource(ImageViewer::default());
    commands.insert_resource(AudioPlayer::default());
}

fn ui_system(
    mut egui_ctx: EguiContexts,
    mut app_state: ResMut<AppState>,
    mut cache: ResMut<TextureCache>,
    channels: ResMut<GuiControllers>,
    mut rodio_player: ResMut<MyRodioHandle>,
    mut servers: ResMut<ServerPanel>,
    mut image_viewer: ResMut<ImageViewer>,
    mut audio_player: ResMut<AudioPlayer>,
    state: Res<MainState>,
) {
    let ctx = egui_ctx.ctx_mut();
    if let MainState::Web = *state {
        egui::TopBottomPanel::bottom("no_match").show(ctx, |ui| {
            if ui.button("Error in Requests").clicked(){
                app_state.error_state = true;
            }
            if app_state.error_state{
                egui::Window::new("ErrorMessages")
                .collapsible(true)
                .max_size((300.,215.))
                .show(&ctx,|ui|{
                    egui::ScrollArea::vertical().show(ui, |ui|{
                        if let Some(client_id) = app_state.selected_source_client {
                            if let Some(web_state) = app_state.browsers_states.get(&client_id) {
                                for msg in web_state.web_pages.default.clone() {
                                    match msg {
                                        DefaultResponse::ERRNOMEDIA | DefaultResponse::ERRNOTEXT =>{
                                            ui.label(RichText::new("No match found for request of AllMediaLinks or AllTextLinks").color(Color32::RED));
                                        },
                                        _=>{}
                                    }
                                }
                                for msg in web_state.web_pages.content.clone() {
                                    match msg {
                                        ContentResponse::NOMEDIAFOUND | ContentResponse::NOTEXTFOUND=>{
                                            ui.label(RichText::new("No match found for request of Media or Text").color(Color32::RED));
                                        },
                                        _=>{}
                                    }
                                }
                            }
                        }
                    });
                    if ui.button("X").clicked() {
                        app_state.error_state = false;
                    }
                });
            }
        });
    
        egui::SidePanel::left("left_panel").show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.heading("Clients");
                for (id, gui_c) in &channels.channels {
                    if !app_state.browsers_states.contains_key(&id) {
                        app_state
                            .browsers_states
                            .insert(*id, WebViewState::default());
                    }
                    if ui.button(format!("WebClient {}", id)).clicked() {
                        app_state.selected_source_client = Some(*id);
                        app_state.browsers_states.entry(*id).or_default();
                        let _ = gui_c.sender.send(WebCommand::GetServersType);
                    }
                }
            });
        });
    
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(client_id) = app_state.selected_source_client {
                if let Some(web_state) = app_state.browsers_states.get(&client_id) {
                    ui.columns(2, |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("first scroll area")
                            .show(&mut ui[0], |ui| {
                                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                                    for msg in web_state.web_pages.default.clone() {
                                        match msg {
                                            DefaultResponse::ALLTEXT(vec) => {
                                                for label in vec {
                                                    ui.label(label.clone());
                                                    if ui.button("->").clicked() {
                                                        if label.contains("all_media") {
                                                            if let Some(server) = servers
                                                                .servers
                                                                .iter()
                                                                .find(|s| s.server_type == MEDIASERVER)
                                                            {
                                                                let _ = channels
                                                                    .channels
                                                                    .get(&client_id)
                                                                    .unwrap()
                                                                    .sender
                                                                    .send(WebCommand::GetAllMedia(
                                                                        server.id,
                                                                    ));
                                                            }
                                                        }
                                                        if label.contains("text") {
                                                            if let Some(server) = servers
                                                                .servers
                                                                .iter()
                                                                .find(|s| s.server_type == TEXTSERVER)
                                                            {
                                                                let _ = channels
                                                                    .channels
                                                                    .get(&client_id)
                                                                    .unwrap()
                                                                    .sender
                                                                    .send(WebCommand::GetText(
                                                                        server.id,
                                                                        label.clone(),
                                                                    ));
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            DefaultResponse::ALLMEDIALINKS(vec) => {
                                                for label in vec {
                                                    ui.label(label.clone());
                                                    if let Some(server) = servers
                                                        .servers
                                                        .iter()
                                                        .find(|s| s.server_type == MEDIASERVER)
                                                    {
                                                        if ui.button("->").clicked() {
                                                            let _ = channels
                                                                .channels
                                                                .get(&client_id)
                                                                .unwrap()
                                                                .sender
                                                                .send(WebCommand::GetMedia(
                                                                    server.id,
                                                                    label.clone(),
                                                                ));
                                                        }
                                                    }
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                    for msg in web_state.web_pages.content.clone() {
                                        match msg {
                                            ContentResponse::TEXT(links) => {
                                                for label in links {
                                                    ui.label(label.clone());
                                                    if ui.button("->").clicked(){
                                                        if label.contains("text/"){
                                                            if let Some(server) = servers
                                                            .servers
                                                            .iter()
                                                            .find(|s| s.server_type == TEXTSERVER)
                                                            {   
                                                                let _ = channels
                                                                .channels
                                                                .get(&client_id)
                                                                .unwrap()
                                                                .sender
                                                                .send(WebCommand::GetText(
                                                                    server.id,
                                                                    label.clone(),
                                                                ));
                                                                info!("SentGetText");
                                                            }
                                                        }
                                                        if label.contains("media/"){
                                                            if let Some(server) = servers
                                                            .servers
                                                            .iter()
                                                            .find(|s| s.server_type == MEDIASERVER)
                                                            {   
                                                                let _ = channels
                                                                .channels
                                                                .get(&client_id)
                                                                .unwrap()
                                                                .sender
                                                                .send(WebCommand::GetMedia(
                                                                    server.id,
                                                                    label.clone(),
                                                                ));
                                                                info!("SentGetMedia");
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                });
                            });
                        egui::ScrollArea::vertical()
                            .id_salt("second scroll area")
                            .show(&mut ui[1], |ui| {
                                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                                    for msg in web_state.web_pages.content.clone() {
                                        match msg {
                                            ContentResponse::MEDIAIMAGE(img) => {
                                                let id = img_hash(&img);
                                                let texture =
                                                    handle_incoming_image(&img, ctx, &mut cache, id);
                                                if ui.button("📷").clicked() {
                                                    image_viewer.open = true;
                                                    image_viewer.current_image_id = Some(id); // <-- your actual image ID here
                                                }
                                                if image_viewer.open
                                                    && image_viewer.current_image_id.is_some()
                                                {
                                                    if let Some(id_1) =
                                                        image_viewer.current_image_id
                                                    {
                                                        if id_1==id {

                                                            egui::Window::new("Image Viewer")
                                                            .collapsible(false)
                                                            .resizable(true)
                                                            .default_size([300.0, 300.0])
                                                            .show(ctx, |ui| {
                                                                if let Some(_d_texture) =
                                                                cache.map.get(&id)
                                                                {
                                                                    let size = texture.size_vec2();
                                                                    ui.add(
                                                                        egui::Image::new(&texture)
                                                                        .fit_to_exact_size(size),
                                                                    );
                                                                } else {
                                                                    ui.label(
                                                                        "Image not found in cache.",
                                                                    );
                                                                }
                                                                
                                                                if ui.button("❌ Close").clicked() {
                                                                    image_viewer.open = false;
                                                                    image_viewer.current_image_id = None;
                                                                }
                                                            });
                                                        }
                                                    }
                                                }
                                            }
                                            ContentResponse::MEDIAUDIO(track) => {
                                                let i = audio_hash(&track);
                                                if ui.button("🎙️").clicked() {
                                                    audio_player.open = true;
                                                    audio_player.current_audio_id = Some(i); // <-- your actual image ID here
                                                }
                                                if audio_player.open
                                                    && audio_player.current_audio_id.is_some()
                                                {
                                                    if let Some(id) = audio_player.current_audio_id {
                                                        if i==id {
                                                            egui::Window::new("Audio")
                                                                .collapsible(false)
                                                                .resizable(true)
                                                                .default_size([100.0, 300.0])
                                                                .show(ctx, |ui| {
                                                                    if ui.button("▶ Play").clicked() {
                                                                        if let Some(track_bytes) =
                                                                            Some(&track.bytes)
                                                                        {
                                                                            let cursor = Cursor::new(
                                                                                track_bytes.clone(),
                                                                            );
                                                                            if let Ok(source) =
                                                                                Decoder::new(cursor)
                                                                            {
                                                                                if let Ok(sink) = Sink::try_new(
                                                                                    &rodio_player.0.handle,
                                                                                ) {
                                                                                    sink.append(source);
                                                                                    rodio_player
                                                                                        .0
                                                                                        .sinks
                                                                                        .insert(i, sink);
                                                                                }
                                                                            } else {
                                                                                error!(
                                                                                    "Failed to decode audio"
                                                                                );
                                                                            }
                                                                        }
                                                                    }
                                                                if ui.button("⏹ Stop").clicked() {
                                                                    if let Some(sink) =
                                                                        rodio_player.0.sinks.remove(&i)
                                                                    {
                                                                        sink.stop();
                                                                    }
                                                                }
                                                                if ui.button("❌ Close").clicked() {
                                                                    audio_player.open = false;
                                                                    audio_player.current_audio_id = None;
                                                                }
                                                            });
                                                        }
                                                    }
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                });
                            });
                    });
                }
            }
        });
    
        egui::SidePanel::right("Text and Media").show(ctx, |ui| {
            if let Some(client_id) = app_state.selected_source_client {
                if ui.button("GET MEDIA LINKS").clicked() {
                    for s in servers.servers.clone() {
                        if s.server_type == MEDIASERVER {
                            let _ = channels
                                .channels
                                .get(&client_id)
                                .unwrap()
                                .sender
                                .send(WebCommand::GetAllMedia(s.id));
                        }
                    }
                }
                ui.separator();
                if ui.button("GET TEXTS LINKS").clicked() {
                    for s in servers.servers.clone() {
                        if s.server_type == TEXTSERVER {
                            let _ = channels
                                .channels
                                .get(&client_id)
                                .unwrap()
                                .sender
                                .send(WebCommand::GetAllText(s.id));
                        }
                    }
                }
            }
        });
    }
    if let Some(cli) = app_state.selected_source_client {
        while let Ok(event) = channels.channels.get(&cli).unwrap().receiver.try_recv() {
            if let Some(view) = app_state.browsers_states.get_mut(&cli) {
                match event {
                    WebEvent::Servers(server_type, id) => {
                        if !servers.servers.iter().any(|s| s.id == id) {
                            servers.servers.push(ServerInfo { id, server_type });
                        }
                    }
                    WebEvent::AllMedia(res) => {
                        info!("{:?}", res.clone());
                        view.web_pages
                            .default
                            .push(DefaultResponse::ALLMEDIALINKS(res.clone()));
                    }
                    WebEvent::AllText(res) => {
                        info!("{:?}", res.clone());
                        view.web_pages
                            .default
                            .push(DefaultResponse::ALLTEXT(res.clone()));
                    }
                    WebEvent::ErrNoAllMedia=>{
                        view.web_pages
                            .default
                            .push(DefaultResponse::ERRNOMEDIA);
                    }
                    WebEvent::Audio(res) => {
                        view.web_pages
                            .content
                            .push(ContentResponse::MEDIAUDIO(res.clone()));
                    }
                    WebEvent::ErrNoAllText=>{
                        view.web_pages
                            .default
                            .push(DefaultResponse::ERRNOTEXT);
                    }
                    WebEvent::Image(res) => {
                        view.web_pages
                            .content
                            .push(ContentResponse::MEDIAIMAGE(res.clone()));
                    }
                    WebEvent::Text(res) => {
                        view.web_pages
                            .content
                            .push(ContentResponse::TEXT(res.clone()));
                    }
                    WebEvent::ErrMediaNotFound => {
                        view.web_pages
                            .content
                            .push(ContentResponse::NOMEDIAFOUND);
                    }
                    WebEvent::ErrTextNotFound => {
                        view.web_pages
                            .content
                            .push(ContentResponse::NOTEXTFOUND);
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
    if let Some(handle) = cache.map.get_mut(&id) {
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

fn audio_hash(track: &AudioSource) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    track.clone().bytes.hash(&mut hasher);
    hasher.finish()
}
