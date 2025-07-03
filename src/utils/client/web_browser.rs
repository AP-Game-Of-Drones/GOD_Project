use crate::frontend::WebCommand;
use crate::frontend::WebEvent;

use super::super::controller::*;
use super::super::fragmentation_handling::DefaultsRequest;
use super::super::fragmentation_handling::*;
use super::super::topology::*;
use crossbeam_channel::*;
use rand::rngs::OsRng;
use rand::*;
use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};
use wg_2024::{network::*, packet::*};

const TEXTSERVER: u8 = 1;
const MEDIASERVER: u8 = 2;

#[derive(Debug)]
enum ProcessWebResult {
    MEDIA,
    TEXT,
    ALLMEDIA,
    ALLTEXT,
    NOTEXT,
    NOMEDIA,
    NOTEXTS,
    NOMEDIAS,
    SERVERFOUND,
    NOSERVER,
    ERR,
}

impl super::Client for WebBrowser {}
pub struct WebBrowser {
    pub id: NodeId,                                   // Unique identifier for the client
    pub controller_send: Sender<NodeEvent>, // Sender for communication with the controller
    pub controller_recv: Receiver<NodeCommand>, // Receiver for commands from the controller
    pub packet_recv: Receiver<Packet>,      // Receiver for incoming packets
    pub packet_send: HashMap<NodeId, Sender<Packet>>, // Map of packet senders for neighbors
    pub flood_ids: HashSet<(u64, NodeId)>,  // Set to track flood IDs for deduplication
    pub client_topology: super::super::topology::Topology, // topology built by flooding
    pub holder_sent: HashMap<(u64, NodeId), Vec<Packet>>, //fragment holder of sent messages, use session_id,src_id tuple as key
    pub holder_frag_index: HashMap<(u64, NodeId), Vec<u64>>, //fragment indices holder for received packets, use session_id,src_id tuple as key
    pub holder_rec: HashMap<(u64, NodeId), Vec<u8>>, //data holder of received messages, use session_id,src_id tuple as key
    pub pre_processed: Option<((u64, NodeId), Message)>,
    pub sent: HashMap<(u64, u8), Message>,
    pub text_servers: Vec<NodeId>,
    pub media_servers: Vec<NodeId>,
    pub media: HashMap<(u64, u8), Message>,
    pub text: HashMap<(u64, u8), Vec<String>>,
    pub gui_command_receiver: Receiver<WebCommand>,
    pub gui_event_sender: Sender<WebEvent>,
}

impl WebBrowser {
    pub fn new(
        id: NodeId,
        controller_send: Sender<NodeEvent>,
        controller_recv: Receiver<NodeCommand>,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<NodeId, Sender<Packet>>,
        gui_command_receiver: Receiver<WebCommand>,
        gui_event_sender: Sender<WebEvent>,
    ) -> Self {
        Self {
            id,
            controller_send,
            controller_recv,
            packet_recv,
            packet_send,
            flood_ids: HashSet::new(),
            client_topology: Topology::new(),
            holder_sent: HashMap::new(),
            holder_frag_index: HashMap::new(),
            holder_rec: HashMap::new(),
            pre_processed: None,
            text_servers: Vec::new(),
            media_servers: Vec::new(),
            sent: HashMap::new(),
            media: HashMap::new(),
            text: HashMap::new(),
            gui_command_receiver,
            gui_event_sender,
        }
    }

    fn handle_packet(&mut self, packet: Packet) {
        match packet.clone().pack_type {
            PacketType::Ack(ack) => {
                // println!("REC ACK IN CHATCLIENT[{}]", self.id);
                match self.recv_ack_n_handle(packet.clone().session_id, ack.clone().fragment_index)
                {
                    Ok(_) => {
                        // println!("Handled Ack");
                    }
                    Err(e) => {
                        println!("{}", e);
                    }
                }
            }
            PacketType::Nack(nack) => {
                // println!("REC NACK IN CHATCLIENT[{}]", self.id);
                match self.recv_nack_n_handle(packet.session_id, nack, &packet.clone()) {
                    Ok(_) => {
                        // println!("Handled Nack");
                    }
                    Err(e) => {
                        println!("{}", e);
                    }
                }
            }
            PacketType::FloodRequest(f_request) => {
                // println!("REC FLOODREQUEST IN CHATCLIENT[{}]", self.id);
                match self.recv_flood_request_n_handle(packet.session_id, &mut f_request.clone()) {
                    Ok(_) => {
                        // println!("Handled FloodReq Client");
                    }
                    Err(e) => {
                        println!("{}", e);
                    }
                }
            }
            PacketType::FloodResponse(f_response) => {
                // println!("REC FLOODRESPONSE IN CHATCLIENT[{}]", self.id);
                match self.recv_flood_response_n_handle(f_response) {
                    Ok(_) => {
                        // println!("Client Topology [{}] : {:?}\n\n",self.id , self.client_topology);
                        // println!("Handled FloodResp in CL\n");
                    }
                    Err(e) => {
                        println!("Err: {}", e);
                    }
                }
            }
            PacketType::MsgFragment(fragment) => {
                // println!("REC MSGFRAGMENT IN CHATCLIENT[{}]", self.id);
                // println!("Hops: {:?}",packet.clone().routing_header.hops);
                match self.recv_frag_n_handle(
                    packet.session_id,
                    packet.clone().routing_header.hops[0],
                    &fragment,
                ) {
                    Some(m) => {
                        // println!("Handled Frag in Client");
                        self.pre_processed = Some((
                            (packet.session_id, packet.clone().routing_header.hops[0]),
                            m.clone(),
                        ));
                        let _processed = self.process_respsonse(
                            m.clone(),
                            packet.session_id,
                            packet.clone().routing_header.hops[0],
                        );
                    }
                    None => {
                        // println!("No message reconstructed yet");
                    }
                }
            }
        }
    }

    fn process_respsonse(
        &mut self,
        response: Message,
        session_id: u64,
        src_id: NodeId,
    ) -> Result<ProcessWebResult, ProcessWebResult> {
        match response.clone() {
            Message::DefaultResponse(df) => match df {
                DefaultResponse::ALLTEXT(res) => {
                    if !res.is_empty() {
                        self.text.insert((session_id, src_id), res.clone());
                        let _ = self.gui_event_sender.send(WebEvent::AllText(res.clone()));
                        Ok(ProcessWebResult::ALLTEXT)
                    } else {
                        Err(ProcessWebResult::NOTEXTS)
                    }
                }
                DefaultResponse::SERVERTYPE(res, id) => {
                    if res == TEXTSERVER {
                        self.text_servers.push(id);
                        let _ = self.gui_event_sender.send(WebEvent::Servers(res, id));
                        Ok(ProcessWebResult::SERVERFOUND)
                    } else if res == MEDIASERVER {
                        self.media_servers.push(id);
                        let _ = self.gui_event_sender.send(WebEvent::Servers(res, id));
                        Ok(ProcessWebResult::SERVERFOUND)
                    } else {
                        Err(ProcessWebResult::NOSERVER)
                    }
                }
                DefaultResponse::ALLMEDIALINKS(res) => {
                    if !res.is_empty() {
                        self.text.insert((session_id, src_id), res.clone());
                        let _ = self.gui_event_sender.send(WebEvent::AllMedia(res.clone()));
                        Ok(ProcessWebResult::ALLMEDIA)
                    } else {
                        Err(ProcessWebResult::NOMEDIAS)
                    }
                }
                DefaultResponse::ERRNOMEDIA => Err(ProcessWebResult::NOMEDIAS),
                DefaultResponse::ERRNOTEXT => Err(ProcessWebResult::NOTEXTS),
                _ => Err(ProcessWebResult::ERR),
            },
            Message::ContentResponse(cr) => match cr.clone() {
                ContentResponse::MEDIAIMAGE(res) => {
                    self.media.insert((session_id, src_id), response.clone());
                    let _ = self.gui_event_sender.send(WebEvent::Image(res.clone()));
                    Ok(ProcessWebResult::MEDIA)
                }
                ContentResponse::MEDIAUDIO(res) => {
                    self.media.insert((session_id, src_id), response.clone());
                    let _ = self.gui_event_sender.send(WebEvent::Audio(res.clone()));
                    Ok(ProcessWebResult::MEDIA)
                }
                ContentResponse::TEXT(res) => {
                    self.text.insert((session_id, src_id), res.clone());
                    let _ = self.gui_event_sender.send(WebEvent::Text(res.clone()));
                    Ok(ProcessWebResult::TEXT)
                }
                ContentResponse::NOMEDIAFOUND => Err(ProcessWebResult::NOMEDIA),
                ContentResponse::NOTEXTFOUND => Err(ProcessWebResult::NOTEXT),
            },
            _ => Err(ProcessWebResult::ERR),
        }
    }

    fn send_new_server_req(&mut self, dst: NodeId) -> Result<(), String> {
        let msg = Message::DefaultsRequest(DefaultsRequest::GETSERVERTYPE);
        self.send_from_web_client(dst, msg.clone())
    }

    fn send_new_all_text_req(&mut self, dst: NodeId) -> Result<(), String> {
        let msg = Message::DefaultsRequest(DefaultsRequest::GETALLTEXT);
        self.send_from_web_client(dst, msg.clone())
    }

    fn send_new_all_media_req(&mut self, dst: NodeId) -> Result<(), String> {
        let msg = Message::DefaultsRequest(DefaultsRequest::GETALLMEDIALINKS);
        self.send_from_web_client(dst, msg.clone())
    }

    fn send_new_text_req(&mut self, dst: NodeId, link: String) -> Result<(), String> {
        let msg = Message::ContentRequest(ContentRequest::GETTEXT(link.clone()));
        self.send_from_web_client(dst, msg.clone())
    }

    fn send_new_media_req(&mut self, dst: NodeId, link: String) -> Result<(), String> {
        let msg = Message::ContentRequest(ContentRequest::GETMEDIA(link.clone()));
        self.send_from_web_client(dst, msg.clone())
    }

    fn send_from_web_client(&mut self, dst: NodeId, msg: Message) -> Result<(), String> {
        match deconstruct_message(msg.clone()) {
            Ok(bytes_res) => {
                let mut fragments: Vec<Fragment> = serialize(bytes_res);
                let mut session_id = 0;
                while self.session_id_alredy_used(session_id) {
                    session_id = rand_session_id();
                }
                self.client_topology.find_all_paths(self.id, dst);
                self.client_topology.set_path_based_on_dst(dst);
                let hops = self.get_hops(dst);
                let packets = fragment_packetization(&mut fragments, hops.clone(), session_id);
                if !packets.is_empty() {
                    self.sent.insert((session_id, self.id), msg.clone());
                    self.holder_sent
                        .insert((session_id, self.id), packets.clone());
                    for pack in packets {
                        match self.send_new_packet(&pack.clone()) {
                            Ok(_) => {
                                let _ = self
                                    .controller_send
                                    .send(NodeEvent::PacketSent(pack.clone()));
                                // println!("Sent new packet in client");
                            }
                            Err(e) => {
                                println!("{}", e);
                            }
                        }
                    }
                    return Ok(());
                } else {
                    return Err("Packets vector empty".to_string());
                }
            }
            Err(e) => {
                println!("{:?}\n\n\n\n", e.clone());
                Err(e)
            }
        }
    }

    pub fn handle_channels(&mut self) {
        loop {
            select! {
                recv(self.packet_recv) -> packet_res => {
                    if let Ok(packet) = packet_res {
                        self.handle_packet(packet.clone());
                    }
                },
                recv(self.controller_recv) -> command_res => {
                    if let Ok(command) = command_res {
                        match command {
                            // NodeCommand::PacketShortcut(packet)=>{
                            //     self.handle_packet(packet);
                            // },
                            NodeCommand::AddSender(id,sender)=>{
                                self.packet_send.insert(id, sender);
                            },
                            NodeCommand::RemoveSender(id)=>{
                                self.packet_send.remove(&id);
                                self.client_topology.remove_node(id);
                            }
                            _=>{}
                        }
                    }
                },
                recv(self.gui_command_receiver) -> gui_command => {
                    if let Ok(command) = gui_command {
                        match command {
                            WebCommand::GetServersType=>{
                                for dst in self.client_topology.get_all_servers() {
                                    let _ =self.send_new_server_req(dst);
                                }
                            },
                            WebCommand::GetAllText(id)=>{
                                bevy::log::info!("All Text to {:?}", id);
                                if self.text_servers.contains(&id) {
                                    let _ = self.send_new_all_text_req(id);
                                }
                            },
                            WebCommand::GetAllMedia(id)=>{
                                bevy::log::info!("All Media to {:?}", id);
                                if self.media_servers.contains(&id) {
                                    let _ = self.send_new_all_media_req(id);
                                }
                            },
                            WebCommand::GetText(id,path)=>{
                                if self.text_servers.contains(&id) {
                                    let _ = self.send_new_text_req(id,path);
                                }
                            },
                            WebCommand::GetMedia(id,path)=>{
                                if self.media_servers.contains(&id) {
                                    let _ = self.send_new_media_req(id,path);
                                }
                            }
                        }
                    }
                },
                default(Duration::from_secs(5)) => {
                    let mut session_id = 0;
                    while self.session_id_alredy_used(session_id) {
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
            Err("No neighbors in Client")
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
                            path_trace: vec![(self.id, NodeType::Client)].clone(),
                        },
                    )) {
                    Ok(_) => {
                        // println!("Sent new flood_req from Client[{}]", self.id);
                        // self.controller_send.send(NodeEvent::PacketSent(packet.clone())).ok();
                    }
                    Err(_) => {
                        println!(
                            "Error_in_Sender from Client[{}] to Drone[{}]",
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
                        self.controller_send
                            .send(NodeEvent::PacketSent(packet.clone()))
                            .ok();
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
            return Err("Client not supposed to receive packet");
        }
    }

    fn send_ack(
        &mut self,
        session_id: u64,
        server_id: &u8,
        fragment_index: u64,
    ) -> Result<(), &str> {
        self.client_topology.find_all_paths(self.id, *server_id);
        self.client_topology.set_path_based_on_dst(*server_id);
        let traces = self.client_topology.get_current_path();
        if let Some(trace) = traces {
            let packet = Packet::new_ack(
                SourceRoutingHeader::with_first_hop(trace.clone()),
                session_id,
                fragment_index,
            );
            if let Some(sender) = self.packet_send.get(&trace[1]) {
                if let Err(_e) = sender.send(packet.clone()) {
                    return Err("Sender error");
                } else {
                    self.controller_send
                        .send(NodeEvent::PacketSent(packet.clone()))
                        .ok();
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
        self.client_topology.find_all_paths(self.id, server_id);
        self.client_topology.set_path_based_on_dst(server_id);
        let traces = self.client_topology.get_current_path();
        if let Some(trace) = traces {
            let packet = Packet::new_fragment(
                SourceRoutingHeader::with_first_hop(trace.clone()),
                session_id,
                fragment.clone(),
            );
            if let Some(sender) = self.packet_send.get(&trace[1]) {
                if let Ok(_) = sender.send(packet.clone()) {
                    self.controller_send
                        .send(NodeEvent::PacketSent(packet.clone()))
                        .ok();
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

    fn send_new_packet(&mut self, packet: &Packet) -> Result<(), &str> {
        if let Some(_sender) = self.packet_send.get(&packet.routing_header.hops[1]) {
            match self
                .packet_send
                .get(&packet.routing_header.hops[1])
                .unwrap()
                .send(packet.clone())
            {
                Ok(_) => {
                    let _ = self
                        .controller_send
                        .send(NodeEvent::PacketSent(packet.clone()));
                    Ok(())
                }
                Err(_) => Err("Something wrong with the sender"),
            }
        } else {
            Err("First hop is wrong")
        }
    }

    pub fn recv_flood_response_n_handle(
        &mut self,
        flood_packet: FloodResponse,
    ) -> Result<(), &str> {
        self.client_topology
            .update_topology((self.id, NodeType::Client), flood_packet.path_trace.clone());
        let serv = self.client_topology.get_all_servers();
        if !serv.is_empty() {
            for s in serv {
                self.client_topology.find_all_paths(self.id, s);
            }
        }
        return Ok(());
    }

    pub fn recv_flood_request_n_handle(
        &mut self,
        session_id: u64,
        flood_packet: &mut FloodRequest,
    ) -> Result<(), &str> {
        let mut path_trace = flood_packet.path_trace.clone();
        path_trace.push((self.id, NodeType::Client));
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
        hops.reverse();
        let flood_response = FloodResponse {
            flood_id: flood_packet.flood_id,
            path_trace: path_trace.clone(),
        };
        let mut new_packet = Packet::new_flood_response(
            SourceRoutingHeader::with_first_hop(hops.clone()),
            session_id,
            flood_response.clone(),
        );
        return self.send_flood_response(&mut new_packet);
    }

    pub fn recv_nack_n_handle(
        &mut self,
        session_id: u64,
        nack: Nack,
        packet: &Packet,
    ) -> Result<(), &str> {
        let flood_id = generate_flood_id(&mut self.flood_ids, self.id);
        let mut session = 0;
        while self.session_id_alredy_used(session) {
            session = rand_session_id();
        }
        match nack.clone().nack_type {
            NackType::DestinationIsDrone => {
                //check route, it shouldn't happen if the routing was done right
                // println!("Dest is drone nacked");
                if let Some(packets) = { self.holder_sent.get(&(session_id, self.id)).cloned() } {
                    for p in packets.clone() {
                        match p.clone().pack_type {
                            PacketType::MsgFragment(f) => {
                                if f.fragment_index == nack.fragment_index {
                                    self.client_topology
                                        .increment_weights_for_node(packet.routing_header.hops[0]);
                                    return self.send_new_generic_fragment(
                                        *p.routing_header.hops.last().unwrap(),
                                        session_id,
                                        f.clone(),
                                    );
                                }
                            }
                            PacketType::Ack(a) => {
                                if a.fragment_index == nack.fragment_index {
                                    self.client_topology
                                        .increment_weights_for_node(packet.routing_header.hops[0]);
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
                self.send_new_flood_request(session, flood_id).ok();

                //update weight of the path used and change it there's one with less
                // println!("Dropped by drone nacked");

                if let Some(fr) = { self.holder_sent.get(&(session_id, self.id)) } {
                    for p in fr.clone() {
                        match p.clone().pack_type {
                            PacketType::MsgFragment(f) => {
                                if f.fragment_index == nack.fragment_index {
                                    self.client_topology
                                        .increment_weights_for_node(packet.routing_header.hops[0]);
                                    return self.send_new_generic_fragment(
                                        *p.routing_header.hops.last().unwrap(),
                                        session_id,
                                        f.clone(),
                                    );
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
                // println!("Error in routing nacked");

                if let Some(packets) = { self.holder_sent.get(&(session_id, self.id)).cloned() } {
                    //Could be a drone in crash mode so remove the node id from topology and update it
                    self.client_topology.remove_node(id);
                    //update the path since it might mean a drone has crashed or bad routing
                    self.client_topology.increment_weights_for_node(id);
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
                // println!("Unexpected rec nacked");

                if let Some(packets) = { self.holder_sent.get(&(session_id, self.id)) } {
                    for p in packets.clone() {
                        match p.clone().pack_type {
                            PacketType::MsgFragment(f) => {
                                if f.fragment_index == nack.fragment_index {
                                    self.client_topology.increment_weights_for_node(id);
                                    return self.send_new_generic_fragment(
                                        *p.routing_header.hops.last().unwrap(),
                                        session_id,
                                        f.clone(),
                                    );
                                }
                            }
                            PacketType::Ack(a) => {
                                if a.fragment_index == nack.fragment_index {
                                    self.client_topology.increment_weights_for_node(id);
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

    pub fn recv_ack_n_handle(&mut self, session_id: u64, fragment_index: u64) -> Result<(), &str> {
        if let Some(holder) = { self.holder_sent.get(&(session_id, self.id)) } {
            if holder.is_empty() && fragment_index == 0 {
                return Err("All fragments of corrisponding message have been received");
            } else if holder.is_empty() && fragment_index != 0 {
                return Err("Not supposed to receive this ACK");
            } else if !holder.is_empty() && fragment_index != 0 {
                let mut i = 0;
                for f in holder.clone() {
                    match f.pack_type {
                        PacketType::MsgFragment(f) => {
                            if f.fragment_index == fragment_index {
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

    pub fn recv_frag_n_handle(
        &mut self,
        session_id: u64,
        src: NodeId,
        frag: &Fragment,
    ) -> Option<Message> {
        self.client_topology.find_all_paths(self.id, src);
        self.client_topology.set_path_based_on_dst(src);
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
            }
            // print!("{} {}\n\n\n", holder.len(), frag.total_n_fragments);
            if holder.len() == (frag.total_n_fragments) as usize {
                if let Some(mut data) = self.holder_rec.get_mut(&(session_id, src)) {
                    remove_trailing_zeros(&mut data);
                    let mut f_serialized = serialize(data.clone());
                    let result = super::super::fragmentation_handling::reconstruct_message(
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
            
            None
        } else {
            self.holder_rec.insert(
                (session_id, src),
                vec![0; (frag.clone().total_n_fragments * 128) as usize],
            );
            // println!("Firsr frag received");
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

    fn session_id_alredy_used(&self, session_id: u64) -> bool {
        if self.holder_sent.contains_key(&(session_id, self.id)) {
            true
        } else {
            false
        }
    }

    fn get_hops(&mut self, dst: u8) -> Option<Vec<u8>> {
        self.client_topology.find_all_paths(self.id, dst);
        self.client_topology.set_path_based_on_dst(dst);
        self.client_topology.get_current_path()
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
            // println!(" Packet {:?}, session : {} ", packet.clone(), session_id);
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

pub mod gui_test {
    use wg_2024::{controller::*, drone::Drone};

    use crate::utils::backup_server::Server;

    use super::*;
    pub fn test_gui_channels_main() {
        use std::collections::HashMap;
        use std::thread;

        let (_c1, c2) = unbounded::<NodeCommand>();
        let (c3, _c4) = unbounded::<NodeEvent>();

        // Shared Channels
        let (c5, c6) = unbounded::<Packet>(); // Channel 1
        let (c5_1, c6_1) = unbounded::<Packet>(); // Channel 2
        let (c5_2, c6_2) = unbounded::<Packet>(); // Channel 3

        let (c7_1, c7) = unbounded::<WebCommand>();
        let (c8, c8_1) = unbounded::<WebEvent>();

        // let (c77_1, c77) = unbounded::<ChatCommand>();
        // let (c88, c88_1) = unbounded::<ChatEvent>();

        // Drone
        let (_c9, c10) = unbounded::<DroneCommand>();
        let (c11, _c12) = unbounded::<DroneEvent>();

        // First ChatClient (id: 1)
        let mut hm1 = HashMap::new();
        hm1.insert(2, c5_1.clone());

        let mut dummy1 = WebBrowser::new(
            1,
            c3.clone(),
            c2.clone(),
            c6.clone(),
            hm1,
            c7.clone(),
            c8.clone(),
        );

        // Second ChatClient (id: 4)
        let (c13, c14) = unbounded::<Packet>(); // Client 2's inbound
        let mut hm2 = HashMap::new();
        hm2.insert(2, c5_1.clone());

        let mut server_1 = Server::new(4, MEDIASERVER, c3.clone(), c2.clone(), c14.clone(), hm2);

        // Server
        let mut server_2 = Server::new(
            3,
            TEXTSERVER,
            c3.clone(),
            c2.clone(),
            c6_2.clone(),
            HashMap::from([(2, c5_1.clone())]),
        );

        // Topology updates
        dummy1.client_topology.update_topology(
            (1, NodeType::Client),
            vec![
                (1, NodeType::Client),
                (2, NodeType::Drone),
                (3, NodeType::Server),
            ],
        );

        dummy1.client_topology.update_topology(
            (1, NodeType::Client),
            vec![
                (1, NodeType::Client),
                (2, NodeType::Drone),
                (4, NodeType::Server),
            ],
        );

        server_1.server_topology.update_topology(
            (4, NodeType::Server),
            vec![
                (4, NodeType::Server),
                (2, NodeType::Drone),
                (1, NodeType::Client),
            ],
        );

        server_2.server_topology.update_topology(
            (3, NodeType::Server),
            vec![
                (3, NodeType::Server),
                (2, NodeType::Drone),
                (1, NodeType::Client),
            ],
        );

        let mut v = Vec::new();
        let channels = crate::frontend::web_gui::GuiChannels::new(c8_1, c7_1);
        // let channels_1 = crate::frontend::web_gui::GuiChannels::new(c88_1, c77_1);
        let hm = HashMap::from([(1, channels)]);
        // Server thread
        v.push(thread::spawn(move || {
            server_2.handle_channels();
        }));

        // Drone thread
        v.push(thread::spawn(move || {
            let mut drone = ap2024_unitn_cppenjoyers_drone::CppEnjoyersDrone::new(
                2,
                c11.clone(),
                c10.clone(),
                c6_1.clone(),
                HashMap::from([(1, c5.clone()), (3, c5_2.clone()), (4, c13.clone())]),
                0.,
            );
            drone.run();
        }));

        // ChatClient 1 thread
        v.push(thread::spawn(move || {
            dummy1.handle_channels();
        }));

        // ChatClient 2 thread
        v.push(thread::spawn(move || {
            server_1.handle_channels();
        }));

        // Launch GUI (main thread)
        crate::frontend::web_gui::main_gui(hm);
    }
}
