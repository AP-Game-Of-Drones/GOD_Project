use super::super::super::frontend::ChatCommand;
use super::super::super::frontend::ChatEvent;
use super::super::controller::*;
use super::super::fragmentation_handling::DefaultsRequest;
use super::super::fragmentation_handling::*;
use crossbeam_channel::*;
use rand::RngCore;
use rand::rngs::OsRng;

use super::super::topology::*;
use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};
use wg_2024::{network::*, packet::*};

const CHATSERVER: u8 = 3;

#[derive(Debug)]
enum ProcessChatResults {
    REGISTERED,
    CHATTERSFOUND,
    MSG,
    SERVERFOUND,
    NOCHATTERS,
    NOSERVER,
    ALREADYREGISTERED,
    TRYAGAIN,
    ERR,
}

impl super::Client for ChatClient {}

#[derive(Clone)]
pub struct ChatClient {
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
    pub registered_to: Vec<NodeId>,
    pub chat_servers: Vec<NodeId>,
    pub chat_contacts: Vec<(NodeId, NodeId)>,
    pub sent: HashMap<(u64, u8), Message>,
    pub received: HashMap<(u64, u8), Message>,
    pub gui_command_receiver: Receiver<ChatCommand>,
    pub gui_event_sender: Sender<ChatEvent>,
}

impl ChatClient {
    pub fn new(
        id: NodeId,
        controller_send: Sender<NodeEvent>,
        controller_recv: Receiver<NodeCommand>,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<NodeId, Sender<Packet>>,
        gui_command_receiver: Receiver<ChatCommand>,
        gui_event_sender: Sender<ChatEvent>,
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
            chat_servers: Vec::new(),
            registered_to: Vec::new(),
            chat_contacts: Vec::new(),
            sent: HashMap::new(),
            received: HashMap::new(),
            gui_command_receiver,
            gui_event_sender,
        }
    }

    pub fn send_register(&mut self, dst: NodeId) -> Result<(), String> {
        let new_req = Message::DefaultsRequest(DefaultsRequest::REGISTER);
        self.send_from_chat_client(dst, new_req)
    }

    pub fn send_get_all_available(&mut self, dst: NodeId) -> Result<(), String> {
        let new_req = Message::DefaultsRequest(DefaultsRequest::GETALLAVAILABLE);
        self.send_from_chat_client(dst, new_req)
    }

    pub fn send_msg_to(&mut self, dst: NodeId, chat_msg: Message) -> Result<(), String> {
        self.send_from_chat_client(dst, chat_msg)
    }

    pub fn send_get_server_type(&mut self, dst: NodeId) -> Result<(), String> {
        let new_req = Message::DefaultsRequest(DefaultsRequest::GETSERVERTYPE);
        self.send_from_chat_client(dst, new_req)
    }

    fn send_from_chat_client(&mut self, dst: NodeId, msg: Message) -> Result<(), String> {
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
                                // println!("Sent packet new in client");
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

    fn process_respsonse(
        &mut self,
        response: Message,
        session_id: u64,
        src_id: NodeId,
    ) -> Result<ProcessChatResults, ProcessChatResults> {
        if let Some(holder) = self.holder_sent.get_mut(&(session_id, self.id)) {
            holder.clear();
            // println!("Message packets inside of holder cleared");
        } else {
            // println!("Message id not inside of holder");
        }
        match response {
            Message::DefaultResponse(df) => match df {
                DefaultResponse::REGISTERED(res, id) => {
                    // println!("Received REGISTERED response");
                    if res {
                        self.registered_to.push(id);
                        let _ = self.send_get_all_available(id);
                        let _ = self.gui_event_sender.send(ChatEvent::Registered(id));
                        Ok(ProcessChatResults::REGISTERED)
                    } else {
                        if self.registered_to.contains(&id) {
                            let _ = self.send_get_all_available(id).ok();
                            Err(ProcessChatResults::ALREADYREGISTERED)
                        } else {
                            Err(ProcessChatResults::TRYAGAIN)
                        }
                    }
                }
                DefaultResponse::SERVERTYPE(res, id) => {
                    // println!("Received SERVERTYPE response");
                    if res == CHATSERVER {
                        self.chat_servers.push(id);
                        // let _ = self.send_register(id);
                        let _ = self.gui_event_sender.send(ChatEvent::Servers(id));
                        Ok(ProcessChatResults::SERVERFOUND)
                    } else {
                        Err(ProcessChatResults::NOSERVER)
                    }
                }
                DefaultResponse::ALLAVAILABLE(res) => {
                    // println!("Received ALLAVAILABLE response");
                    let mut ids = Vec::new();

                    for client_id in res.clone() {
                        if !self.chat_contacts.contains(&(src_id, client_id)) {
                            self.chat_contacts.push((src_id, client_id));
                            ids.push(client_id);
                        }
                    }
                    // println!("Client_ids : {:?}\n\n", ids.clone());

                    let _ = self.gui_event_sender.send(ChatEvent::Clients(res.clone()));
                    Ok(ProcessChatResults::CHATTERSFOUND)
                }
                DefaultResponse::ERRNOAVAILABLE => {
                    // println!("Received ERRNOAVAILABLE response");
                    Err(ProcessChatResults::NOCHATTERS)
                }
                _ => {
                    println!("No DefResp possible");
                    Err(ProcessChatResults::ERR)
                }
            },
            Message::ChatMessages(cm) => {
                self.received
                    .insert((session_id, src_id), Message::ChatMessages(cm.clone()));
                let _ = self.gui_event_sender.send(ChatEvent::NewMessage(cm));
                Ok(ProcessChatResults::MSG)
            }
            Message::Audio(_) => {
                // println!("Audio Response");
                Err(ProcessChatResults::ERR)
            }
            Message::Image(_) => {
                // println!("Image Response");
                Err(ProcessChatResults::ERR)
            }
            Message::String(_) => {
                // println!("String Response");
                Err(ProcessChatResults::ERR)
            }
            Message::ContentRequest(_) => {
                // println!("ContReq Response");
                Err(ProcessChatResults::ERR)
            }
            Message::ContentResponse(_) => {
                // println!("Content Response");
                Err(ProcessChatResults::ERR)
            }
            Message::DefaultsRequest(rsp) => {
                // println!("DefReq Response {:?}", rsp);
                Err(ProcessChatResults::ERR)
            } // _ => {
              //     println!("No possible resp");
              //     Err(ProcessChatResults::ERR)
              // }
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
                            ChatCommand::RegisterTo(dst)=>{
                                self.send_register(dst).ok();
                            }
                            ChatCommand::GetServersType=>{
                                for dst in self.client_topology.get_all_servers() {
                                    self.send_get_server_type(dst).ok();
                                }
                            },
                            ChatCommand::GetClients(id) =>{
                                self.send_get_all_available(id).ok();
                            },
                            ChatCommand::SendMessage(dst,msg) =>{
                                self.send_msg_to(dst, msg).ok();
                            },
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
            let packet = Packet::new_flood_request(
                SourceRoutingHeader::empty_route(),
                session_id,
                FloodRequest {
                    flood_id,
                    initiator_id: self.id,
                    path_trace: vec![(self.id, NodeType::Client)].clone(),
                },
            );
            for neighbors in self.packet_send.clone() {
                match self
                    .packet_send
                    .get(&neighbors.0)
                    .unwrap()
                    .send(packet.clone())
                {
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
        // println!("Path trace in client {:?}", flood_packet.path_trace.clone());
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
                                        .increment_weights_for_node(packet.routing_header.hops[1]);
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
                                        .increment_weights_for_node(packet.routing_header.hops[1]);
                                    return self.send_ack(
                                        session_id,
                                        p.routing_header.hops.last().unwrap(),
                                        a.fragment_index,
                                    );
                                }
                            }
                            _ => {
                                return Err("Packet shouldn't have produced a nack");
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
                                        .increment_weights_for_node(packet.routing_header.hops[1]);
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
                //Could be a drone in crash mode so remove the node id from topology and update it
                self.client_topology.remove_node(id);

                if let Some(packets) = { self.holder_sent.get(&(session_id, self.id)).cloned() } {
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
        if let Some(holder) = { self.holder_sent.get_mut(&(session_id, self.id)) } {
            if holder.is_empty(){
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

#[cfg(test)]
mod tests {

    use super::*;
    use crossbeam_channel::unbounded;

    #[test]
    fn test_send_chat_client() {
        let (_c1, c2) = unbounded::<NodeCommand>();
        let (c3, _c4) = unbounded::<NodeEvent>();
        let (c5, c6) = unbounded::<Packet>();
        let (_, c7) = unbounded::<ChatCommand>();
        let (c8, _) = unbounded::<ChatEvent>();
        let mut hm = HashMap::new();
        hm.insert(1, c5);
        let mut dummy = ChatClient::new(0, c3, c2, c6, hm, c7, c8);
        dummy
            .client_topology
            .update_topology((0, NodeType::Client), vec![(1, NodeType::Server)]);
        let fragments =
            Message::ChatMessages(ChatMessages::CHATSTRING(1, 0, 1, "Hello".to_string()));

        let res = dummy.send_from_chat_client(1, fragments.clone());

        match res {
            Ok(_) => {
                // println!("Sent");
            }
            Err(e) => {
                println!("{:?}", e);
            }
        }
        assert_eq!(1, 2);
    }

    #[test]
    fn test_recv_fragmnt() {
        let (_c1, c2) = unbounded::<NodeCommand>();
        let (c3, _c4) = unbounded::<NodeEvent>();
        let (c5, c6) = unbounded::<Packet>();
        let (_, c7) = unbounded::<ChatCommand>();
        let (c8, _) = unbounded::<ChatEvent>();
        let mut hm = HashMap::new();
        hm.insert(1, c5);

        let mut dummy = ChatClient::new(0, c3, c2, c6, hm, c7, c8);
        dummy
            .client_topology
            .update_topology((0, NodeType::Client), vec![(1, NodeType::Server)]);

        let res = dummy.send_get_all_available(1);

        match res {
            Ok(_) => {
                println!("Sent");
            }
            Err(e) => {
                println!("{:?}", e);
            }
        }
        assert_eq!(1, 2);
    }

    #[test]
    fn test_ack() {
        let (_c1, c2) = unbounded::<NodeCommand>();
        let (c3, _c4) = unbounded::<NodeEvent>();
        let (c5, c6) = unbounded::<Packet>();
        let (_, c7) = unbounded::<ChatCommand>();
        let (c8, _) = unbounded::<ChatEvent>();
        let mut hm = HashMap::new();
        hm.insert(1, c5);

        let mut dummy = ChatClient::new(0, c3, c2, c6, hm, c7, c8);
        dummy
            .client_topology
            .update_topology((0, NodeType::Client), vec![(1, NodeType::Server)]);

        let res = dummy.send_ack(1, &1, 2);

        match res {
            Ok(_) => {
                println!("Sent");
            }
            Err(e) => {
                println!("{:?}", e);
            }
        }
        assert_eq!(1, 2);
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
        let (_c5_3, _c6_3) = unbounded::<Packet>(); // Channel 4

        let (c7_1, c7) = unbounded::<ChatCommand>();
        let (c8, c8_1) = unbounded::<ChatEvent>();

        let (c77_1, c77) = unbounded::<ChatCommand>();
        let (c88, c88_1) = unbounded::<ChatEvent>();

        // Drone
        let (_c9, c10) = unbounded::<DroneCommand>();
        let (c11, _c12) = unbounded::<DroneEvent>();

        // First ChatClient (id: 1)
        let mut hm1 = HashMap::new();
        hm1.insert(2, c5_1.clone());

        let mut dummy1 = ChatClient::new(
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

        let mut dummy2 = ChatClient::new(
            4,
            c3.clone(),
            c2.clone(),
            c14.clone(),
            hm2,
            c77.clone(),
            c88.clone(),
        );

        // Server
        let mut server = Server::new(
            3,
            CHATSERVER,
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

        dummy2.client_topology.update_topology(
            (4, NodeType::Client),
            vec![
                (4, NodeType::Client),
                (2, NodeType::Drone),
                (3, NodeType::Server),
            ],
        );

        server.server_topology.update_topology(
            (3, NodeType::Server),
            vec![
                (3, NodeType::Server),
                (2, NodeType::Drone),
                (1, NodeType::Client),
            ],
        );

        server.server_topology.update_topology(
            (3, NodeType::Server),
            vec![
                (3, NodeType::Server),
                (2, NodeType::Drone),
                (4, NodeType::Client),
            ],
        );

        let mut v = Vec::new();
        let channels = crate::frontend::chat_gui::GuiChannels::new(c8_1, c7_1);
        let channels_1 = crate::frontend::chat_gui::GuiChannels::new(c88_1, c77_1);
        let hm = HashMap::from([(1, channels), (4, channels_1)]);
        // Server thread
        v.push(thread::spawn(move || {
            server.handle_channels();
        }));

        // Drone thread
        v.push(thread::spawn(move || {
            let mut drone = d_r_o_n_e_drone::MyDrone::new(
                2,
                c11.clone(),
                c10.clone(),
                c6_1.clone(),
                HashMap::from([(1, c5.clone()), (3, c5_2.clone()), (4, c13.clone())]),
                0.00,
            );
            drone.run();
        }));

        // ChatClient 1 thread
        v.push(thread::spawn(move || {
            dummy1.handle_channels();
        }));

        // ChatClient 2 thread
        v.push(thread::spawn(move || {
            dummy2.handle_channels();
        }));

        // Launch GUI (main thread)
        crate::frontend::chat_gui::main_gui(hm);
    }
}
