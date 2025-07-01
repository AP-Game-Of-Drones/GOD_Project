use super::fragmentation_handling::DefaultsRequest;
use super::fragmentation_handling::*;
use super::simulation_controller::*;
use bevy::audio::AudioSource;
use bevy::log::info;
use crossbeam_channel::*;

use rand::RngCore;
use rand::rngs::OsRng;

use super::topology::*;
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::BufRead,
    sync::Arc,
    time::Duration,
};
use wg_2024::{network::*, packet::*};

pub const TEXTSERVER: u8 = 1;
pub const MEDIASERVER: u8 = 2;
pub const CHATSERVER: u8 = 3;

include!(concat!(env!("OUT_DIR"), "/build_constants.rs"));

const ALLTEXT: &str = "/assets/web/text/all_text_links.txt";
const ALLMEDIA: &str = "/assets/web/text/all_media_links.txt";

pub trait Servers: Sized + Send + Sync {}
impl Servers for Server {}

#[derive(Clone)]
pub struct Server {
    pub id: NodeId, // Unique identifier for the client
    pub serv_type: u8,
    pub controller_send: Sender<NodeEvent>, // Sender for communication with the controller
    pub controller_recv: Receiver<NodeCommand>, // Receiver for commands from the controller
    pub packet_recv: Receiver<Packet>,      // Receiver for incoming packets
    pub packet_send: HashMap<NodeId, Sender<Packet>>, // Map of packet senders for neighbors
    pub flood_ids: HashSet<(u64, NodeId)>,  // Set to track flood IDs for deduplication
    pub server_topology: super::topology::Topology, // topology built by flooding
    pub holder_sent: HashMap<(u64, NodeId), Vec<Packet>>, //fragment holder of sent messages, use session_id,src_id tuple as key
    pub holder_frag_index: HashMap<(u64, NodeId), Vec<u64>>, //fragment indices holder for received packets, use session_id,src_id tuple as key
    pub holder_rec: HashMap<(u64, NodeId), Vec<u8>>, //data holder of received messages, use session_id,src_id tuple as key
    pub pre_processed: Option<((u64, NodeId), Message)>,
    pub sent: HashMap<(u64, u8), Message>,
    pub received: HashMap<(u64, u8), Message>,
    chatters: HashSet<NodeId>,
}

impl Server {
    pub fn new(
        id: NodeId,
        serv_type: u8,
        controller_send: Sender<NodeEvent>,
        controller_recv: Receiver<NodeCommand>,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<NodeId, Sender<Packet>>,
    ) -> Self {
        Self {
            id,
            serv_type,
            controller_send,
            controller_recv,
            packet_recv,
            packet_send,
            flood_ids: HashSet::new(),
            server_topology: Topology::new(),
            holder_sent: HashMap::new(),
            holder_frag_index: HashMap::new(),
            holder_rec: HashMap::new(),
            pre_processed: None,
            sent: HashMap::new(),
            received: HashMap::new(),
            chatters: HashSet::new(),
        }
    }

    fn get_type(&self) -> u8 {
        self.serv_type
    }

    fn is_text_server(&self) -> bool {
        self.serv_type == TEXTSERVER
    }

    fn is_media_server(&self) -> bool {
        self.serv_type == MEDIASERVER
    }

    fn is_chat_server(&self) -> bool {
        self.serv_type == CHATSERVER
    }

    fn get_chatters(&self) -> Vec<NodeId> {
        // println!("\n\nGotChatters: {:?}\n\n", self.chatters.clone());
        self.chatters.clone().into_iter().map(|id| id).collect()
    }

    fn handle_req(&mut self, request: Message, src_id: NodeId, session_id: u64) {
        self.sent.insert((session_id, self.id), request.clone());
        match request.clone() {
            Message::DefaultsRequest(df) => match &df {
                DefaultsRequest::GETSERVERTYPE => {
                    // println!("Received Type Request");
                    self.send_from_server(
                        src_id,
                        Message::DefaultResponse(DefaultResponse::new_server_type_rsp(
                            self.get_type(),
                            self.id,
                        )),
                    );
                }
                DefaultsRequest::GETALLAVAILABLE => {
                    if self.is_chat_server() {
                        self.send_from_server(
                            src_id,
                            Message::DefaultResponse(DefaultResponse::new_available_rsp(
                                self.get_chatters(),
                            )),
                        );
                    } else {
                        self.send_from_server(
                            src_id,
                            Message::DefaultResponse(DefaultResponse::new_no_available_rsp()),
                        );
                    }
                }
                DefaultsRequest::REGISTER => {
                    if self.is_chat_server() {
                        self.chatters.insert(src_id);
                        self.send_from_server(
                            src_id,
                            Message::DefaultResponse(DefaultResponse::new_registered_rsp(
                                true, self.id,
                            )),
                        );
                        // println!("Chatters {:?}\n", self.chatters);
                    } else {
                        self.send_from_server(
                            src_id,
                            Message::DefaultResponse(DefaultResponse::new_registered_rsp(
                                false, self.id,
                            )),
                        );
                    }
                }
                DefaultsRequest::GETALLTEXT => {
                    if self.is_text_server() {
                        let all_text = get_all((PROJECT_DIR.to_string() + ALLTEXT).as_str());
                        self.send_from_server(
                            src_id,
                            Message::DefaultResponse(DefaultResponse::new_all_text_rsp(all_text)),
                        );
                    } else {
                        self.send_from_server(
                            src_id,
                            Message::DefaultResponse(DefaultResponse::new_err_no_text_rsp()),
                        );
                    }
                }
                DefaultsRequest::GETALLMEDIALINKS => {
                    let all_text = get_all((PROJECT_DIR.to_string() + ALLMEDIA).as_str());
                    info!("{:?}", all_text.clone());
                    if self.is_media_server() {
                        self.send_from_server(
                            src_id,
                            Message::DefaultResponse(DefaultResponse::new_all_media_rsp(
                                all_text.clone(),
                            )),
                        );
                    } else {
                        self.send_from_server(
                            src_id,
                            Message::DefaultResponse(DefaultResponse::new_err_no_media_rsp()),
                        );
                    }
                }
                _ => {}
            },
            Message::ContentRequest(cr) => match &cr {
                ContentRequest::GETMEDIA(path) => {
                    if self.is_media_server() {
                        if path.contains("image") {
                            let media = image::open(path.as_str());
                            info!(
                                "Received request for {},{:?}",
                                path,
                                std::env::current_dir()
                            );
                            if let Ok(img) = media {
                                self.send_from_server(
                                    src_id,
                                    Message::ContentResponse(ContentResponse::MEDIAIMAGE(img)),
                                );
                            } else {
                                info!("Failed to open {}, {:?}", path, media.err());
                                self.send_from_server(
                                    src_id,
                                    Message::ContentResponse(ContentResponse::NOMEDIAFOUND),
                                );
                            }
                        } else if path.contains("audio") {
                            let track_bytes = fs::read(path.as_str());
                            if let Ok(bytes) = track_bytes {
                                let track = AudioSource {
                                    bytes: Arc::from(bytes),
                                };
                                self.send_from_server(
                                    src_id,
                                    Message::ContentResponse(ContentResponse::MEDIAUDIO(track)),
                                );
                            } else {
                                self.send_from_server(
                                    src_id,
                                    Message::ContentResponse(ContentResponse::NOMEDIAFOUND),
                                );
                            }
                        }
                    }
                }
                ContentRequest::GETTEXT(path) => {
                    if self.is_text_server() {
                        let text = get_all(path.as_str());
                        self.send_from_server(
                            src_id,
                            Message::ContentResponse(ContentResponse::TEXT(text)),
                        );
                    } else {
                        self.send_from_server(
                            src_id,
                            Message::ContentResponse(ContentResponse::NOTEXTFOUND),
                        );
                    }
                }
            },
            Message::ChatMessages(cm) => match &cm {
                ChatMessages::CHATAUDIO(src, srv, dst, track) => {
                    if self.is_chat_server() {
                        self.send_from_server(
                            *dst,
                            Message::ChatMessages(ChatMessages::new_audio_msg(
                                *src,
                                *srv,
                                *dst,
                                track.clone(),
                            )),
                        );
                    }
                }
                ChatMessages::CHATSTRING(src, srv, dst, string) => {
                    if self.is_chat_server() {
                        self.send_from_server(
                            *dst,
                            Message::ChatMessages(ChatMessages::new_string_msg(
                                *src,
                                *srv,
                                *dst,
                                string.clone(),
                            )),
                        );
                    }
                }
                ChatMessages::CHATIMAGE(src, srv, dst, image) => {
                    if self.is_chat_server() {
                        self.send_from_server(
                            *dst,
                            Message::ChatMessages(ChatMessages::new_image_msg(
                                *src,
                                *srv,
                                *dst,
                                image.clone(),
                            )),
                        );
                    }
                }
            },
            _ => {}
        }
    }

    pub fn handle_channels(&mut self) {
        loop {
            select! {
                recv(self.packet_recv) -> packet_res => {
                    if let Ok(packet) = packet_res {
                        match packet.clone().pack_type {
                            PacketType::Ack(ack) => {
                                // println!("REC ACK IN SERVER[{}]",self.id);
                                match self.recv_ack_n_handle(packet.clone().session_id,ack.clone().fragment_index){
                                    Ok(_) => {
                                        // println!("Handled Ack");
                                    },
                                    Err(e) => {
                                        println!("{}",e);
                                    }
                                }
                            },
                            PacketType::Nack(nack) => {
                                // println!("REC NACK IN SERVER[{}]",self.id);
                                match self.recv_nack_n_handle(packet.session_id, nack, &packet.clone()) {
                                    Ok(_) => {
                                        // println!("Handled Nack");
                                    },
                                    Err(e) => {
                                        println!("{}",e);
                                    }
                                }
                            },
                            PacketType::FloodRequest(f_request) => {
                                // println!("REC FLOODREQUEST IN SERVER[{}]",self.id);
                                match self.recv_flood_request_n_handle(packet.session_id, &mut f_request.clone()) {
                                    Ok(_) => {
                                        // println!("Handled FloodReq in Server & Sent new Flood Response");
                                    },
                                    Err(e) => {
                                        println!("{}",e);
                                    }
                                }
                            },
                            PacketType::FloodResponse(f_response) => {
                                // println!("REC FLOODRESPONSE IN SERVER[{}]",self.id);

                                match self.recv_flood_response_n_handle(f_response) {
                                    Ok(_) => {
                                        // println!("Server Topology: {:?}\n\n",self.server_topology);
                                        // println!("Handled FloodResp");
                                    },
                                    Err(e) => {
                                        println!("{}",e);
                                    }
                                }
                            },
                            PacketType::MsgFragment(fragment) => {
                                // println!("REC MSGFRAGMENT IN SERVER[{}]",self.id);
                                match self.recv_frag_n_handle(packet.session_id, packet.clone().routing_header.hops[0], &fragment) {
                                    Some(m) => {
                                        // println!("msg in server {} \n \n ",self.id);
                                        self.pre_processed=Some(((packet.session_id,packet.clone().routing_header.hops[0]),m.clone()));
                                        self.handle_req(m.clone(), packet.clone().routing_header.hops[0], packet.session_id);
                                    },
                                    None => {
                                        // println!("No message reconstructed");
                                    }
                                }
                            }
                        }
                    }
                },
                recv(self.controller_recv) -> command_res => {
                    if let Ok(command) = command_res {
                        match command {
                         _=>{}
                        }
                    }
                },
                default(Duration::from_secs(15)) => {
                    let mut session_id = 0;
                    while self.session_id_already_used(session_id) {
                        session_id = rand_session_id();
                    }
                    let flood_id = generate_flood_id(&mut self.flood_ids,self.id);
                    let _= self.send_new_flood_request(session_id, flood_id);
                }
            }
        }
    }

    pub fn send_new_flood_request(&mut self, session_id: u64, flood_id: u64) -> Result<(), &str> {
        if self.packet_send.is_empty() {
            Err("No neighbors in Server")
        } else {
            for neighbors in self.packet_send.clone() {
                match self
                    .packet_send
                    .get(&neighbors.0)
                    .unwrap()
                    .send(Packet::new_flood_request(
                        SourceRoutingHeader::empty_route(),
                        session_id,
                        FloodRequest {
                            flood_id,
                            initiator_id: self.id,
                            path_trace: vec![(self.id, NodeType::Server)],
                        },
                    )) {
                    Ok(_) => {
                        // println!("Sent new flood_req from server[{}]", self.id);
                    }
                    Err(_) => {
                        println!(
                            "Error in sending new flood_req from server[{}] to drone[{}]",
                            self.id, neighbors.0
                        );
                    }
                }
            }
            Ok(())
        }
    }

    fn send_flood_response(&mut self, packet: &mut Packet) -> Result<(), &str> {
        if packet.routing_header.hops[packet.routing_header.hop_index - 1] == self.id {
            if let Some(sender) = self
                .packet_send
                .get(&packet.routing_header.hops[packet.routing_header.hop_index])
            {
                match sender.send(packet.clone()) {
                    Ok(_) => {
                        return Ok(());
                    }
                    Err(_) => {
                        return Err("Error in sender of client");
                    }
                }
            } else {
                return Err("Error in routing");
            }
        } else {
            return Err("Server not supposed to receive packet");
        }
    }

    fn send_ack(
        &mut self,
        session_id: u64,
        dst: &u8,
        fragment_index: u64,
    ) -> Result<(), &str> {
        self.server_topology.find_all_paths(self.id, *dst);
        self.server_topology.set_path_based_on_dst(*dst);
        let traces = self.server_topology.get_current_path();
        if let Some(trace) = traces {
            if let Some(sender) = self.packet_send.get(&trace[1]) {
                if let Err(_e) = sender.send(Packet::new_ack(
                    SourceRoutingHeader::with_first_hop(trace.clone()),
                    session_id,
                    fragment_index,
                )) {
                    return Err("Sender error");
                } else {
                    return Ok(());
                }
            } else {
                return Err("No sender found");
            }
        } else {
            return Err("No current path");
        }
    }

    fn send_new_generic_fragment(
        &mut self,
        server_id: NodeId,
        session_id: u64,
        fragment: Fragment,
    ) -> Result<(), &str> {
        self.server_topology.find_all_paths(self.id, server_id);
        self.server_topology.set_path_based_on_dst(server_id);
        let traces = self.server_topology.get_current_path();
        if let Some(trace) = traces {
            if let Some(sender) = self.packet_send.get(&trace[1]) {
                if let Ok(_) = sender.send(Packet::new_fragment(
                    SourceRoutingHeader::with_first_hop(trace.clone()),
                    session_id,
                    fragment.clone(),
                )) {
                    return Ok(());
                } else {
                    return Err("Error in sender");
                }
            } else {
                return Err("Sender not found");
            }
        } else {
            return Err("No current path");
        }
    }

    fn send_from_server(&mut self, dst: NodeId, response: Message) {
        self.server_topology.find_all_paths(self.id, dst);
        self.server_topology.set_path_based_on_dst(dst);
        self.server_topology.get_current_path();
        let mut session_id = 0;
        while self.session_id_already_used(session_id) {
            session_id = rand_session_id();
        }

        let hops = self.get_hops(dst);
        // println!("Hops in server: {:?}",hops);

        let bytes_res = deconstruct_message(response.clone());
        if let Ok(bytes) = bytes_res {
            let mut frags = serialize(bytes);
            let packets = fragment_packetization(&mut frags, hops, session_id);
            self.holder_sent
                .insert((session_id, self.id), packets.clone());
            for packet in packets {
                self.send_new_packet(packet).ok();
            }
        }
    }

    fn send_new_packet(&mut self, packet: Packet) -> Result<(), &str> {
        if let Some(sender) = self
            .packet_send
            .get(&packet.routing_header.hops[packet.routing_header.hop_index])
        {
            match sender.send(packet.clone()) {
                Ok(_) => {
                    let _ = self
                        .controller_send
                        .send(NodeEvent::SentPacket(packet.clone()));
                    Ok(())
                }
                Err(_) => Err("Something wrong with the sender"),
            }
        } else {
            Err("First hop is wrong")
        }
    }

    fn recv_flood_response_n_handle(
        &mut self,
        flood_packet: FloodResponse,
    ) -> Result<(), &str> {
        self.server_topology
            .update_topology((self.id, NodeType::Server), flood_packet.path_trace.clone());
        // println!("Path trace in server {:?}", flood_packet.path_trace.clone());
        let cl = self.server_topology.get_all_clients();
        if !cl.is_empty() {
            for c in cl {
                self.server_topology.find_all_paths(self.id, c);
            }
        }
        // println!("Server Topology\n\n {:?}", self.server_topology.clone());
        return Ok(());
    }

    fn recv_flood_request_n_handle(
        &mut self,
        session_id: u64,
        flood_packet: &mut FloodRequest,
    ) -> Result<(), &str> {
        let mut path_trace = flood_packet.path_trace.clone();
        path_trace.push((self.id, NodeType::Server));
        if !self
            .flood_ids
            .contains(&(flood_packet.flood_id, flood_packet.initiator_id))
        {
            self.flood_ids
                .insert((flood_packet.flood_id, flood_packet.initiator_id));
        }
        let mut hops = path_trace
            .clone()
            .into_iter()
            .map(|(id, _)| id)
            .collect::<Vec<u8>>();
        // println!("hops: {:?}", hops.clone());
        hops.reverse();
        // println!("hops reversed: {:?}", hops.clone());
        let flood_response = FloodResponse {
            flood_id: flood_packet.flood_id,
            path_trace: path_trace.clone(),
        };
        let mut new_packet = Packet::new_flood_response(
            SourceRoutingHeader::with_first_hop(hops.clone()),
            session_id,
            flood_response.clone(),
        );
        // println!("Flood Response: {:?}", new_packet.clone());
        return self.send_flood_response(&mut new_packet);
    }

    fn recv_nack_n_handle(
        &mut self,
        session_id: u64,
        nack: Nack,
        packet: &Packet,
    ) -> Result<(), &str> {
        let flood_id = generate_flood_id(&mut self.flood_ids, self.id);
        let mut session = 0;
        while self.session_id_already_used(session) {
            session = rand_session_id();
        }
        self.send_new_flood_request(session, flood_id).ok();
        match nack.clone().nack_type {
            NackType::DestinationIsDrone => {
                //check route, it shouldn't happen if the routing was done right

                if let Some(packets) = { self.holder_sent.get(&(session_id, self.id)).cloned() } {
                    info!("Received Nack for session {}->Line 594",session_id);
                    for p in packets.clone() {
                        match p.clone().pack_type {
                            PacketType::MsgFragment(f) => {
                                if f.fragment_index == nack.fragment_index {
                                    self.server_topology
                                        .increment_weights_for_node(packet.routing_header.hops[0]);
                                    self.server_topology.update_current_path();
                                    return self.send_new_generic_fragment(
                                        *p.routing_header.hops.last().unwrap(),
                                        session_id,
                                        f.clone(),
                                    );
                                }
                            }
                            PacketType::Ack(a) => {
                                if a.fragment_index == nack.fragment_index {
                                    self.server_topology
                                        .increment_weights_for_node(packet.routing_header.hops[0]);
                                    self.server_topology.update_current_path();
                                    return self.send_ack(
                                        session_id,
                                        p.routing_header.hops.last().unwrap(),
                                        a.fragment_index,
                                    );
                                }
                            }
                            _ => {
                                return Err("Packet should have not produced a nack");
                            }
                        }
                    }
                } else {
                    return Err("No matching session_id");
                }
            }
            NackType::Dropped => {
                //update weight of the path used and change it there's one with less
                if let Some(fr) = { self.holder_sent.get(&(session_id, self.id)) } {
                    info!("Received Nack for session {}: Line->634",session_id);

                    for p in fr.clone() {
                        match p.clone().pack_type {
                            PacketType::MsgFragment(f) => {
                                info!("FragmentIndex {} NAckIndex{}: Line->639",f.fragment_index,nack.fragment_index);
                                if f.fragment_index == nack.fragment_index {
                                    self.server_topology
                                        .increment_weights_for_node(packet.routing_header.hops[0]);
                                    self.server_topology.update_current_path();
                                    return self.send_new_generic_fragment(
                                        *p.routing_header.hops.last().unwrap(),
                                        session_id,
                                        f.clone(),
                                    )
                                }
                            }
                            _ => {
                                return Err("Packet should not have been dropped");
                            }
                        }
                    }
                } else {
                    return Err("No matching session_id");
                }
            }
            NackType::ErrorInRouting(id) => {
                if let Some(packets) = { self.holder_sent.get(&(session_id, self.id)).cloned() } {
                    info!("Received Nack for session {} : Line->662 ",session_id);

                    //update the path since it might mean a drone has crashed or bad routing
                    self.server_topology.increment_weights_for_node(id);
                    self.server_topology.update_current_path();
                    for p in packets.clone() {
                        match p.clone().pack_type {
                            PacketType::MsgFragment(f) => {
                                if f.fragment_index == nack.fragment_index {
                                    return self.send_new_generic_fragment(
                                        *p.routing_header.hops.last().unwrap(),
                                        session_id,
                                        f.clone(),
                                    );
                                }
                            }
                            PacketType::Ack(a) => {
                                if a.fragment_index == nack.fragment_index {
                                    return self.send_ack(
                                        session_id,
                                        p.routing_header.hops.last().unwrap(),
                                        a.fragment_index,
                                    );
                                }
                            }
                            _ => {
                                return Err("Packet should have not produced a Nack");
                            }
                        }
                    }
                } else {
                    return Err("No matching session_id");
                }
            }
            NackType::UnexpectedRecipient(id) => {
                //shouldn't happen, if it happens update paths and update topology
                if let Some(packets) = { self.holder_sent.get(&(session_id, self.id)) } {
                    info!("Received Nack for session {}: Line->700",session_id);

                    for p in packets.clone() {
                        match p.clone().pack_type {
                            PacketType::MsgFragment(f) => {
                                if f.fragment_index == nack.fragment_index {
                                    self.server_topology.increment_weights_for_node(id);
                                    self.server_topology.update_current_path();
                                    return self.send_new_generic_fragment(
                                        *p.routing_header.hops.last().unwrap(),
                                        session_id,
                                        f.clone(),
                                    );
                                }
                            }
                            PacketType::Ack(a) => {
                                if a.fragment_index == nack.fragment_index {
                                    self.server_topology.increment_weights_for_node(id);
                                    self.server_topology.update_current_path();
                                    return self.send_ack(
                                        session_id,
                                        p.routing_header.hops.last().unwrap(),
                                        a.fragment_index,
                                    );
                                }
                            }
                            _ => {
                                return Err("Packet should have not produced a Nack");
                            }
                        }
                    }
                } else {
                    return Err("No matching session_id");
                }
            }
        }
        return Err("No match found for session_id and fragment_index");
    }

    fn recv_ack_n_handle(
        &mut self,
        session_id: u64,
        fragment_index: u64,
    ) -> Result<(), &str> {
        if let Some(holder) = { self.holder_sent.get(&(session_id, self.id)) } {
            if holder.is_empty() && fragment_index == 0 {
                return Err("All fragments of corrisponding message have been received");
            } else if holder.is_empty() && fragment_index != 0 {
                return Err("Not supposed to receive this ACK");
            } else if !holder.is_empty() && fragment_index != 0 {
                let mut i = 0;
                for f in holder.clone() {
                    match f.pack_type {
                        PacketType::Ack(a) => {
                            if a.fragment_index == fragment_index {
                                break;
                            }
                            i += 1;
                        }
                        _ => {}
                    }
                }

                self.holder_sent
                    .get_mut(&(session_id, self.id))
                    .unwrap()
                    .remove(i);
                return Ok(());
            } else {
                return Err("Fragment Index was not supposed to be 0");
            }
        } else {
            return Err("No matching key found for Ack");
        }
    }

    fn recv_frag_n_handle(
        &mut self,
        session_id: u64,
        src: NodeId,
        frag: &Fragment,
    ) -> Option<Message> {
        self.server_topology.find_all_paths(self.id, src);
        self.server_topology.set_path_based_on_dst(src);
        self.send_ack(session_id, &src, frag.fragment_index).ok();
        if let Some(holder) = self.holder_frag_index.get_mut(&(session_id, src)) {
            if !holder.contains(&frag.fragment_index) {
                // println!("Fragm n: 1  < n <  tot");
                let target = self.holder_rec.get_mut(&(session_id, src)).unwrap();
                update_holder_rec(
                    target,
                    &frag.data,
                    frag.length as usize,
                    (session_id, src),
                    frag.fragment_index as usize,
                );
                holder.push(frag.fragment_index);

                // print!("{} {}\n\n\n", holder.len(), frag.total_n_fragments);
                if holder.len() == (frag.total_n_fragments) as usize {
                    if let Some(mut data) = self.holder_rec.get_mut(&(session_id, src)) {
                        remove_trailing_zeros(&mut data);
                        let mut f_serialized = serialize(data.clone());
                        let result = super::fragmentation_handling::reconstruct_message(
                            data[0],
                            &mut f_serialized,
                        );

                        if let Ok(msg) = result {
                            self.holder_rec.remove(&(session_id, src));
                            self.holder_frag_index.remove(&(session_id, src));
                            self.pre_processed = Some(((session_id, src), msg.clone()));
                            // println!("Message Reconstructed");
                            return Some(msg.clone());
                        } else {
                            self.pre_processed = None;
                            // println!("Message reconstruction failed");
                            return None;
                        }
                    }
                }
            }
            None
        } else {
            self.holder_rec.insert(
                (session_id, src),
                vec![0; (frag.clone().total_n_fragments * 128) as usize],
            );
            // println!("First frag received");
            update_holder_rec(
                &mut self.holder_rec.get_mut(&(session_id, src)).unwrap(),
                &frag.data,
                frag.length as usize,
                (session_id, src),
                frag.fragment_index as usize,
            );
            self.holder_frag_index
                .insert((session_id, src), [frag.fragment_index].to_vec());
            return None;
        }
    }

    fn session_id_already_used(&self, session_id: u64) -> bool {
        if self.holder_sent.contains_key(&(session_id, self.id)) {
            true
        } else {
            false
        }
    }

    fn get_hops(&mut self, dst: u8) -> Option<Vec<u8>> {
        self.server_topology.find_all_paths(self.id, dst);
        self.server_topology.update_current_path();
        self.server_topology.set_path_based_on_dst(dst);
        self.server_topology.get_current_path()
    }
}

fn rand_session_id() -> u64 {
    let mut bytes = [0u8; 8];
    OsRng.fill_bytes(&mut bytes);
    u64::from_ne_bytes(bytes)
}
fn fragment_packetization(
    fragments: &mut Vec<Fragment>,
    hops: Option<Vec<u8>>,
    session_id: u64,
) -> Vec<Packet> {
    let mut vec = Vec::new();
    fragments.sort_by_key(|f| f.fragment_index);

    if hops.is_some() {
        for f in fragments {
            let packet = Packet::new_fragment(
                SourceRoutingHeader::with_first_hop(hops.clone().unwrap()),
                session_id,
                f.clone(),
            );
            vec.push(packet);
        }
    }
    vec
}

fn remove_trailing_zeros(vec: &mut Vec<u8>) {
    if let Some(pos) = vec.iter().rposition(|&x| x != 0) {
        vec.truncate(pos + 1);
    } else {
        vec.clear(); // If the vector contains only zeros, clear it
    }
}

fn generate_flood_id(flood_ids: &mut HashSet<(u64, NodeId)>, id: NodeId) -> u64 {
    let mut rng = 1;
    while !flood_ids.insert((rng, id)) {
        rng = rand::random::<u64>();
    }
    rng
}

fn update_holder_rec(
    target: &mut Vec<u8>,
    data: &[u8],
    length: usize,
    _key: (u64, NodeId),
    index: usize,
) {
    let mut finish_pos = ((index - 1) * 128) + 1;

    // Handle special case for the first fragment
    if index == 1 {
        target[0] = data[0];
    } else {
        if length < 128 {
            finish_pos = finish_pos - (128 - length);
        }

        // Copy the fragment data into the correct position in the target vector
        target[finish_pos - length..finish_pos].copy_from_slice(&data[..length]);
    }
}

fn get_all(path: &str) -> Vec<String> {
    let res = read_file_to_lines(path);
    if let Ok(vec) = res { vec } else { vec![] }
}

fn read_file_to_lines(file_path: &str) -> Result<Vec<String>, std::io::Error> {
    let file = File::open(file_path)?;
    let reader = std::io::BufReader::new(file);

    // Handle potential errors when collecting lines
    reader
        .lines()
        .collect::<Result<Vec<String>, std::io::Error>>() // Use `Result::collect` to handle errors
}

#[cfg(test)]
mod testing {
    use crate::utils::backup_server::{ALLMEDIA, get_all};

    #[test]
    fn test_open() {
        println!("{:?}", get_all(ALLMEDIA));
        assert_eq!(1, 2);
    }
}
