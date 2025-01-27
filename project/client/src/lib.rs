#![allow(dead_code, unused)]
use app::Plugin;
use audio::AudioSource;
use bevy::*;
use codecs::png::PngDecoder;
use controller::*;
use crossbeam_channel::*;
use fragmentation_handling::DefaultsRequest;
use fragmentation_handling::*;
use image::*;
use io::Reader;
use bevy::prelude::Button;
use render::{render_resource::Extent3d, texture::ImageFormat};
use std::{
    collections::{HashMap, HashSet},
    io::Cursor,
    mem::swap,
    ops::Deref,
    sync::Arc,
    thread,
    time::Duration,
};
use topology::*;
use utils::FloatOrd;
use wg_2024::{network::*, packet::*};
use window::{PrimaryWindow, Window};
use rand::*;

/// Todo :
///   APIs: GET, POST, DELETE, ...
///     -login to chatserver
///         fn register(&self);
///     -getter for other clients to chat with
///         fn get_contacts(&self)->&[Nodeid];
///     -register as available for chatting
///         -setters to available or unavailable
///             fn available(&mut self);
///             fn unavailable(&mut self);
///     -getters for medias and text;
///         fn get_all_media()->File;
///         fn get_all_text()->File;
///     -send message
///     -delete web results or simple refresh, think about cache like environment or full stack saving
///         We will have assets inside the git repo, for server holding, instead for client media save
///         we could have a tmp directory, that we could delete on app exit;
///     -...
///   Inner Backend:
///     -Topology and source routing handling;
///         Refractor Topology divided from net_init
///     -Message handling;
///         - Assembler & fragmentation of messages; V
///     -Error handling;
///     -If drone crashed cause path errors, do the client notify the sim contr or does
///         it already know and it's working on it  ?
///     - Strongly codependant on servers so we hope to have a good server end;
///  GUI:
///     -bevy dependency
///     -A Primary window for choosing and init( maybe thinkin it as a desktop)
///     -so diffrent apps, browser and chatting app( two icons that open two diffrent windows)
///     -So a gui for the browser and one for the chattapp, that work with the api described prev.
///     

#[derive(Clone)]
pub enum ClientType {
    ChatClient(Client),
    WebBrowser(Client),
}
// Client structure representing a client node in the network
#[derive(Clone)]
pub struct Client {
    pub id: NodeId,                                   // Unique identifier for the client
    pub controller_send: Sender<NodeEvent>, // Sender for communication with the controller
    pub controller_recv: Receiver<NodeCommand>, // Receiver for commands from the controller
    pub packet_recv: Receiver<Packet>,      // Receiver for incoming packets
    pub packet_send: HashMap<NodeId, Sender<Packet>>, // Map of packet senders for neighbors
    pub flood_ids: HashSet<u64>,            // Set to track flood IDs for deduplication
    pub client_topology: topology::Topology, // topology built by flooding
    pub holder_sent: HashMap<(u64,NodeId), Vec<Fragment>>, //fragment holder of sent messages, use session_id,src_id tuple as key
    pub holder_frag_index: HashMap<(u64,NodeId), Vec<u64>>, //fragment indices holder, use session_id,src_id tuple as key
    pub holder_rec: HashMap<(u64,NodeId), Vec<u8>>, //data holder of received messages, use session_id,src_id tuple as key
    pub current_path: Vec<NodeId>,               //current path used
}

#[derive(Clone)]
pub struct WebBrowser<T>
where
    T: Fragmentation<T> + Assembler<T>,
{
    pub client_type: ClientType,
    pub history: HashMap<u8, String>,
    pub media: Vec<T>,
    pub gui: WebGui,
}

impl<T> WebBrowser<T>
where
    T: Fragmentation<T> + Assembler<T>,
{
    pub fn new(
        id: NodeId,
        controller_send: Sender<NodeEvent>,
        controller_recv: Receiver<NodeCommand>,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<NodeId, Sender<Packet>>,
    ) -> Self {
        Self {
            client_type: ClientType::WebBrowser(Client::new(
                id,
                controller_send,
                controller_recv,
                packet_recv,
                packet_send,
            )),
            history: HashMap::new(),
            media: Vec::new(),
            gui: WebGui,
        }
    }
}

#[derive(Clone)]
pub struct WebGui;
impl Plugin for WebGui {
    fn build(&self, app: &mut bevy::prelude::App) {}
}

#[derive(Clone)]
pub struct ChatClient<T>
where
    T: Fragmentation<T> + Assembler<T>,
{
    pub client_type: ClientType,
    pub sent: HashMap<u8, T>,
    pub received: HashMap<u8, T>,
    pub gui: ChatGui,
}

impl<T> ChatClient<T>
where
    T: Fragmentation<T> + Assembler<T>,
{
    pub fn new(
        id: NodeId,
        controller_send: Sender<NodeEvent>,
        controller_recv: Receiver<NodeCommand>,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<NodeId, Sender<Packet>>,
    ) -> Self {
        Self {
            client_type: ClientType::ChatClient(Client::new(
                id,
                controller_send,
                controller_recv,
                packet_recv,
                packet_send,
            )),
            sent: HashMap::new(),
            received: HashMap::new(),
            gui: ChatGui,
        }
    }
}

#[derive(Clone)]
pub struct ChatGui;


fn remove_trailing_zeros(vec: &mut Vec<u8>) {
    if let Some(pos) = vec.iter().rposition(|&x| x != 0) {
        vec.truncate(pos + 1);
    } else {
        vec.clear(); // If the vector contains only zeros, clear it
    }
}


fn generate_flood_id(flood_ids: &mut HashSet<u64>) -> u64 {
    if flood_ids.is_empty() {
        flood_ids.insert(1);
        1
    } else {
        let mut rng = 1;
        while !flood_ids.insert(rng) {
            rng = rand::thread_rng().gen::<u64>();
        }
        rng
    }
}

fn update_holder_rec(holder_rec: &mut HashMap<(u64,u8),Vec<u8>>,data: &[u8], offset: usize, lenght: usize, key: (u64,NodeId)) {
    for i in offset..offset+lenght {
        holder_rec.get_mut(&key).unwrap()[i]=data[i-offset]
    }
}

impl Client {
    pub fn new(
        id: NodeId,
        controller_send: Sender<NodeEvent>,
        controller_recv: Receiver<NodeCommand>,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<NodeId, Sender<Packet>>,
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
            current_path: Vec::new(),
        }
    }

    pub fn run(&mut self) {
        loop {
            select_biased! {
                recv(self.controller_recv) -> command_res => {
                    if let Ok(command) = command_res {
                        match command {
                         _=>{}   
                        }
                    }
                },
                recv(self.packet_recv) -> packet_res => {
                    if let Ok(packet) = packet_res {
                        match packet.clone().pack_type {
                            PacketType::Ack(ack) => {
                                self.recv_ack_n_handle(packet.clone().routing_header.hops[0],packet.clone().session_id,ack.clone().fragment_index);
                            },
                            PacketType::Nack(nack) => {
                                self.recv_nack_n_handle(packet.session_id, *packet.routing_header.hops.last().unwrap(), nack, &packet.clone());
                            },
                            PacketType::FloodRequest(f_request) => {
                                self.recv_flood_request_n_handle(packet.session_id, packet.clone(), f_request);
                            },
                            PacketType::FloodResponse(f_response) => {
                                self.recv_flood_response_n_handle(packet.session_id, &mut packet.clone(), f_response);
                            },
                            PacketType::MsgFragment(fragment) => {
                                self.recv_frag_n_handle(packet.session_id, packet.clone().routing_header.hops[0], &fragment);
                            },
                        }
                    }
                }
            }
        }
    }

    fn send_new_flood_request(&self, session_id: u64, flood_id: u64) -> Result<(()), &str> {
        for neighbors in self.packet_send.clone() {
            if let Err(res) =
                self.packet_send
                    .get(&neighbors.0)
                    .unwrap()
                    .send(Packet::new_flood_request(
                        SourceRoutingHeader::empty_route(),
                        session_id,
                        FloodRequest::new(flood_id, self.id),
                    ))
            {
                return Err("One or more neighbors was not found");
            }
        }
        Ok(())
    }

    fn send_flood_response(&self, session_id: u64, packet: &mut Packet) -> Result<(()), &str> {
        if packet.routing_header.hops[packet.routing_header.hop_index] == self.id {
            packet.routing_header.increase_hop_index();
            if let Some(sender) = self
                .packet_send
                .get(&packet.routing_header.current_hop().unwrap())
            {
                if let Err(_e) = sender.send(packet.clone()) {
                    return Err("Error in sender of client");
                }
            } else {
                return Err("Error in routing");
            }
            return Ok(());
        } else {
            return Err("Client not supposed to receive packet");
        }
    }

    fn send_ack(
        &self,
        session_id: u64,
        first_hop: &u8,
        hops: Vec<u8>,
        fragment_index: u64,
    ) -> Result<(()), &str> {
        if let Some(sender) = self.packet_send.get(first_hop) {
            if let Err(e) = sender.send(Packet::new_ack(
                SourceRoutingHeader::with_first_hop(hops),
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
    }

    fn send_new_default_request(
        &self,
        server_id: NodeId,
        session_id: u64,
        request: DefaultsRequest,
    ) -> Result<(()), &str> {
        let paths = self.client_topology.shortest_path(self.id, server_id);
        let bytes =
            <DefaultsRequest as fragmentation_handling::Fragmentation<DefaultsRequest>>::fragment(
                request,
            );
        let fragments = fragmentation_handling::serialize(bytes);
        let mut packets = Vec::new();

        if let Some(trace) = paths {
            for fr in fragments {
                packets.push(Packet::new_fragment(
                    SourceRoutingHeader::with_first_hop(trace.clone()),
                    session_id,
                    fr,
                ));
            }
            if trace[0] == self.id {
                for packet in packets {
                    self.packet_send
                        .get(&trace[1])
                        .unwrap()
                        .send(packet.clone())
                        .expect("Sender error");
                }
                Ok(())
            } else {
                for mut packet in packets {
                    let mut vec = [self.id].to_vec();
                    packet.routing_header.hops.append(&mut vec);
                    packet.routing_header.hops = vec.clone();
                    self.packet_send
                        .get(&trace[1])
                        .unwrap()
                        .send(packet.clone())
                        .expect("Sender error");
                }
                Ok(())
            }
        } else {
            Err("Error in source routing")
        }
    }

    fn send_new_string_query(
        &self,
        server_id: NodeId,
        session_id: u64,
        query: String,
    ) -> Result<(()), &str> {
        let paths = self.client_topology.shortest_path(self.id, server_id);
        let bytes = <String as fragmentation_handling::Fragmentation<String>>::fragment(query);
        let fragments = fragmentation_handling::serialize(bytes);
        let mut packets = Vec::new();

        if let Some(trace) = paths {
            for fr in fragments {
                packets.push(Packet::new_fragment(
                    SourceRoutingHeader::with_first_hop(trace.clone()),
                    session_id,
                    fr,
                ));
            }
            if trace[0] == self.id {
                for packet in packets {
                    self.packet_send
                        .get(&trace[1])
                        .unwrap()
                        .send(packet.clone())
                        .expect("Sender error");
                }
                Ok(())
            } else {
                for mut packet in packets {
                    let mut vec = [self.id].to_vec();
                    packet.routing_header.hops.append(&mut vec);
                    packet.routing_header.hops = vec.clone();
                    self.packet_send
                        .get(&trace[1])
                        .unwrap()
                        .send(packet.clone())
                        .expect("Sender error");
                }
                Ok(())
            }
        } else {
            Err("Error in source routing")
        }
    }

    fn send_new_generic_fragment(
        &self,
        server_id: NodeId,
        session_id: u64,
        fragment: Fragment,
    ) -> Result<(()), &str> {
        if let Some(trace) = self.client_topology.shortest_path(self.id, server_id) {
            if let Some(sender) = self.packet_send.get(&trace[0]) {
                if let Ok(_) = sender.send(Packet::new_fragment(
                    SourceRoutingHeader::with_first_hop(trace.clone()),
                    session_id,
                    fragment,
                )) {
                    return Ok(());
                } else {
                    return Err("Error in sender");
                }
            } else {
                return Err("Sender not found");
            }
        } else {
            return Err("No path found");
        }
    }

    fn recv_flood_response_n_handle(
        &mut self,
        session_id: u64,
        packet: &mut Packet,
        flood_packet: FloodResponse,
    ) -> Result<(()), &str> {
        if packet.routing_header.hops[0] == self.id {
            self.client_topology
                .update_topology((self.id, NodeType::Client), flood_packet.path_trace);
            return Ok(());
        } else {
            if let Err(e) = self.send_flood_response(packet.session_id, packet) {
                return Err(e);
            }
            return Ok(());
        }
    }

    fn recv_flood_request_n_handle(
        &mut self,
        session_id: u64,
        packet: Packet,
        flood_packet: FloodRequest,
    ) {
        if self.flood_ids.contains(&flood_packet.flood_id) {
            self.send_flood_response(session_id, &mut packet.clone());
        } else {
            for neigbor in self.packet_send.clone() {
                if neigbor.0 != flood_packet.path_trace.last().unwrap().0 {
                    neigbor.1.send(packet.clone());
                }
            }
        }
    }

    fn recv_nack_n_handle(&mut self, session_id: u64, dst: u8, nack: Nack, packet: &Packet) -> Result<(()), &str> {
        let flood_id = generate_flood_id(&mut self.flood_ids);
        self.send_new_flood_request(session_id, flood_id).ok();
        match nack.nack_type {
            NackType::DestinationIsDrone => {
                //check route, it shouldn't happen if the routing was done right
            }
            NackType::Dropped => {
                //update weight of the path used and change it there's one with less
                if let Some(fr) = self.holder_sent.get(&(session_id,self.id)) {
                    for f in fr.clone() {
                        if f.fragment_index == nack.fragment_index {
                            return self.send_new_generic_fragment(*self.current_path.last().unwrap(), session_id, f.clone());
                        }
                    }
                } else {
                    return Err("No matching session_id");
                }
            }
            NackType::ErrorInRouting(id) => {
                //update the topology since it might mean a drone has crashed or bad routing
            }
            NackType::UnexpectedRecipient(id) => {
                //shouldn't happen, if it happens update paths and update topology 
                
            }
        }
        return Err("No match found for session_id and fragment_index");
    }

    fn recv_ack_n_handle(&mut self, src: NodeId, session_id: u64 , fragment_index: u64) -> Result<&str, &str> {
        if let Some(holder) = self.holder_sent.get_mut(&(session_id,src)){
            if holder.is_empty() && fragment_index == 0{
                return Ok("All fragments of corrisponding message have been received");
            } else if holder.is_empty()&&fragment_index!=0{
                return Err("Not supposed to receive this ACK");
            } else if !holder.is_empty() && fragment_index!=0{
                let mut i = 0;
                for f in holder.clone() {
                    if f.fragment_index==fragment_index {
                        break;
                    }
                    i+=1;
                }
                holder.remove(i);
                return Ok("Received Ack and processed");
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
    ) -> Result<fragmentation_handling::Message, String> {
        let mut res = Err("Not reconstructed yet".to_string());
    
        // Scope the mutable borrow of `self.holder_frag_index`
        if let Some(holder) = self.holder_frag_index.get_mut(&(session_id, src)) {
            if !holder.contains(&frag.fragment_index) {
                let offset = frag.length as usize * frag.fragment_index as usize;
    
                // Scope ends here
                update_holder_rec(&mut self.holder_rec,&frag.data, offset, frag.length as usize, (session_id, src));
                holder.push(frag.fragment_index);
            }
            if holder.len() == frag.total_n_fragments as usize {
                if let Some(data) = self.holder_rec.get_mut(&(session_id, src)) {
                    remove_trailing_zeros(data);
    
                    res = fragmentation_handling::reconstruct_message(
                        data[0],
                        &mut serialize(data.to_vec()),
                    );
    
                    if res.is_ok() {
                        self.holder_rec.remove(&(session_id, src));
                        self.holder_frag_index.remove(&(session_id, src));
                    }
                }
            }
        } else {
            self.holder_rec.insert(
                (session_id, src),
                vec![0; (frag.clone().total_n_fragments * 128) as usize],
            );
            let offset = frag.length as usize * frag.fragment_index as usize;
            update_holder_rec(&mut self.holder_rec, &frag.data, offset, frag.length as usize, (session_id, src));
        }
    
        self.send_ack(session_id, &self.id, [].to_vec(), frag.fragment_index);
        res
    }
    
}
