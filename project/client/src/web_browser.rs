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
use wg_2024::{network::*, packet::*};
use window::{PrimaryWindow, Window};
use rand::*;

const TEXTSERVER: u8 = 1;
const MEDIASERVER: u8 = 2;
const CHATSERVER: u8 = 3;

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
    ERR
}

pub struct WebBrowser
{
    pub id: NodeId,                                   // Unique identifier for the client
    pub controller_send: Sender<NodeEvent>, // Sender for communication with the controller
    pub controller_recv: Receiver<NodeCommand>, // Receiver for commands from the controller
    pub packet_recv: Receiver<Packet>,      // Receiver for incoming packets
    pub packet_send: HashMap<NodeId, Sender<Packet>>, // Map of packet senders for neighbors
    pub flood_ids: HashSet<u64>,            // Set to track flood IDs for deduplication
    pub client_topology: topology::Topology, // topology built by flooding
    pub holder_sent: HashMap<(u64,NodeId), Vec<Packet>>, //fragment holder of sent messages, use session_id,src_id tuple as key
    pub holder_frag_index: HashMap<(u64,NodeId), Vec<u64>>, //fragment indices holder for received packets, use session_id,src_id tuple as key
    pub holder_rec: HashMap<(u64,NodeId), Vec<u8>>, //data holder of received messages, use session_id,src_id tuple as key
    pub pre_processed: Option<((u64,NodeId),Message)>,
    pub history: HashMap<(u64,u8), Message>,
    pub text_servers: Vec<NodeId>,
    pub media_servers: Vec<NodeId>,
    pub media: HashMap<(u64,u8), Message>,
    pub text: HashMap<(u64,u8), Vec<String>>,
}


impl WebBrowser{

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
            pre_processed: None,
            text_servers: Vec::new(),
            media_servers: Vec::new(),
            history: HashMap::new(),
            media: HashMap::new(),
            text: HashMap::new(),
        }
    }

    pub fn run(&mut self){
        let mut session_id = rand_session_id();
        while self.session_id_alredy_used(session_id){
            session_id = rand_session_id()
        }
        let mut flood_id = generate_flood_id(&mut self.flood_ids);
        self.send_new_flood_request(session_id, flood_id);


        let mut session_id = rand_session_id();
        while self.session_id_alredy_used(session_id){
            session_id = rand_session_id()
        }
        let mut flood_id = generate_flood_id(&mut self.flood_ids);
        self.send_new_flood_request(session_id, flood_id);

        self.handle_channels(None) ;
    }

    fn process_respsonse(&mut self, response: Message, session_id: u64, src_id: NodeId) -> Result<ProcessWebResult,ProcessWebResult> {
        if self.holder_sent.get(&(session_id,self.id)).is_some() {
            match response.clone() {
                Message::DefaultResponse( df ) => {
                    match df {
                        DefaultResponse::ALLTEXT(res) => {
                            if !res.is_empty() {
                                self.text.insert((session_id,src_id),res.clone());
                                Ok(ProcessWebResult::ALLTEXT)
                            } else {
                               Err(ProcessWebResult::NOTEXTS)
                            }
                        },
                        DefaultResponse::SERVERTYPE(res,id) => {
                            if res == TEXTSERVER {
                                self.text_servers.push(id);
                                Ok(ProcessWebResult::SERVERFOUND)
                            } else if res == MEDIASERVER{
                                self.media_servers.push(id);
                                Ok(ProcessWebResult::SERVERFOUND)
                            } else {
                                Err(ProcessWebResult::NOSERVER)
                            }
                        },
                        DefaultResponse::ALLMEDIALINKS(res) => {
                            if !res.is_empty() {
                                self.text.insert((session_id,src_id),res.clone());
                                Ok(ProcessWebResult::ALLMEDIA)
                            } else {
                               Err(ProcessWebResult::NOMEDIAS)
                            }
                        },
                        DefaultResponse::ERRNOMEDIA => {
                            Err(ProcessWebResult::NOMEDIAS)
                        },
                        DefaultResponse::ERRNOTEXT => {
                            Err(ProcessWebResult::NOTEXTS)
                        },
                        _ => {
                            Err(ProcessWebResult::ERR)
                        }
                    }
                },
                Message::ContentResponse(cr) => {
                    match cr.clone() {
                        ContentResponse::MEDIAIMAGE(_) => {
                            self.media.insert((session_id,src_id), response.clone());
                            Ok(ProcessWebResult::MEDIA)
                        },
                        ContentResponse::MEDIAUDIO(_) => {
                            self.media.insert((session_id,src_id), response.clone());
                            Ok(ProcessWebResult::MEDIA)
                        },
                        ContentResponse::TEXT(res)=> {
                            self.text.insert((session_id,src_id),res.clone());
                            Ok(ProcessWebResult::ALLMEDIA)
                        },
                        ContentResponse::NOMEDIAFOUND => {
                            Err(ProcessWebResult::NOMEDIA)
                        },
                        ContentResponse::NOTEXTFOUND => {
                            Err(ProcessWebResult::NOTEXT)
                        }
                        _=> {
                            Err(ProcessWebResult::ERR)
                        }   
                    }
                },
                _ => {
                    Err(ProcessWebResult::ERR)
                }
            }
        } else {
            Err(ProcessWebResult::ERR)
        }
    }

    fn send_new_server_req(&mut self,dst:NodeId)-> Result<(()),String> {
        let msg = Message::DefaultsRequest(DefaultsRequest::GETSERVERTYPE);
        self.send_from_web_client(dst, msg.clone())
    }

    fn send_new_all_text_req(&mut self,dst:NodeId)-> Result<(()),String> {
        let msg = Message::DefaultsRequest(DefaultsRequest::GETALLTEXT);
        self.send_from_web_client(dst, msg.clone())           
    }

    fn send_new_all_media_req(&mut self,dst:NodeId)-> Result<(()),String> {
        let msg = Message::DefaultsRequest(DefaultsRequest::GETALLMEDIALINKS);
        self.send_from_web_client(dst, msg.clone())           
    }

    fn send_new_text_req(&mut self,dst:NodeId,link: String)-> Result<(()),String> {
        let msg = Message::ContentRequest(ContentRequest::GETTEXT(link.clone()));
        self.send_from_web_client(dst, msg.clone())           
    }

    fn send_new_media_req(&mut self,dst:NodeId,link: String)-> Result<(()),String> {
        let msg = Message::ContentRequest(ContentRequest::GETMEDIA(link.clone()));
        self.send_from_web_client(dst, msg.clone())           
    }

    fn send_from_web_client(&mut self ,dst:NodeId, msg: Message,)->Result<(()),String> {
        let bytes = deconstruct_message(msg.clone());
        self.client_topology.find_all_paths(self.id,dst);
        self.client_topology.set_path_based_on_dst(dst);
        match bytes {
            Ok(bytes_res) => {
                let mut fragments: Vec<Fragment> = serialize(bytes_res);
                let mut session_id = 0;
                while !self.session_id_alredy_used(session_id) {
                    session_id = rand_session_id();
                }
                let packets = fragment_packetization(&mut fragments.clone(), self.get_hops(dst) , session_id);
                self.history.insert(((session_id,self.id)), msg.clone());
                self.holder_sent.insert((session_id,self.id),packets.clone());
                for pack in packets {
                    match self.send_new_packet(pack.clone()) {
                        Ok(_) => {
                        },
                        Err(e) => {return Err(e.to_string());}
                    } 
                }
                Ok(())
            },
            Err(e) => {
                Err(e)
            }
        }
    }    
    
    pub fn send_new_flood_request(&mut self, session_id: u64, flood_id: u64)->Result<(()),&str> {
        if self.packet_send.is_empty(){
            Err("No neighbors in Client")
        } else {
            for neighbors in self.packet_send.clone() {
                self.packet_send
                    .get(&neighbors.0)
                    .unwrap()
                    .send(Packet::new_flood_request(
                        SourceRoutingHeader::empty_route(),
                        session_id,
                        FloodRequest::new(flood_id, self.id),
                    )).unwrap();
            }
            Ok(())
        }
    }

    fn send_flood_response(&self, session_id: u64, packet: &mut Packet) -> Result<(()), &str> {
        if packet.routing_header.hops[packet.routing_header.hop_index] == self.id {
            packet.routing_header.hop_index+=1;
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
        &mut self,
        session_id: u64,
        server_id: &u8,
        fragment_index: u64,
    ) -> Result<(()), &str> {
        self.client_topology.find_all_paths(self.id,*server_id);
        self.client_topology.set_path_based_on_dst(*server_id);
        let traces = self.client_topology.get_current_path();
        if let Some(trace) =  traces {
        if let Some(sender) = self.packet_send.get(&trace[1]) {
            if let Err(e) = sender.send(Packet::new_ack(
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
    ) -> Result<(()), &str> {
        self.client_topology.find_all_paths(self.id,server_id);
        self.client_topology.set_path_based_on_dst(server_id);
        let traces = self.client_topology.get_current_path();
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

    fn send_new_packet(&mut self, packet: Packet)->Result<(()),&str> {
        if let Some(sender) = self.packet_send.get(&packet.routing_header.hops[1]){
            match sender.send(packet.clone()) {
                Ok(_)=> Ok(()),
                Err(_) => Err("Something wrong with the sender")
            }
        } else {
            Err("First hop is wrong")
        }
    }


    pub fn handle_channels (&mut self, id: Option<NodeId>)  {
        let mut counter = 0;
        loop{
            if counter == 100000 {
                let mut session_id = rand_session_id();
                while self.session_id_alredy_used(session_id){
                    session_id = rand_session_id();
                }
                let flood_id = generate_flood_id(&mut self.flood_ids);
                self.send_new_flood_request(session_id, flood_id);
                println!("SENT {}",counter);
            }
            counter+=1;
            select_biased! {
                recv(self.packet_recv) -> packet_res => {
                    if let Ok(packet) = packet_res {
                        match packet.clone().pack_type {
                            PacketType::Ack(ack) => {
                                match self.recv_ack_n_handle(packet.clone().routing_header.hops[0],packet.clone().session_id,ack.clone().fragment_index){
                                    Ok(_) => {
                                        println!("Handled Ack");
                                    },
                                    Err(e) => {
                                        println!("{}",e);
                                    }
                                }
                            },
                            PacketType::Nack(nack) => {
                                match self.recv_nack_n_handle(packet.session_id, nack, &packet.clone()) {
                                    Ok(_) => {
                                        println!("Handled Nack");
                                    },
                                    Err(e) => {
                                        println!("{}",e);
                                    }
                                }
                            },
                            PacketType::FloodRequest(f_request) => {
                                println!("REC FLOODREQUEST IN {}",self.id);
                                match self.recv_flood_request_n_handle(packet.session_id, packet.clone(), &mut f_request.clone()) {
                                    Ok(_) => {
                                        println!("Handled FloodReq Client");
                                    },
                                    Err(e) => {
                                        println!("{}",e);
                                    }
                                }
                            },
                            PacketType::FloodResponse(f_response) => {
                                match self.recv_flood_response_n_handle(packet.session_id, &mut packet.clone(), f_response) {
                                    Ok(_) => {
                                        
                                        println!("Handled FloodResp in C\n");
                                    },
                                    Err(e) => {
                                        println!("{}",e);
                                    }
                                }
                            },
                            PacketType::MsgFragment(fragment) => {
                                match self.recv_frag_n_handle(packet.session_id, packet.clone().routing_header.hops[0], &fragment) {
                                    Some(m) => {
                                        println!("Handled Frag");
                                        self.pre_processed=Some(((packet.session_id,packet.clone().routing_header.hops[0]),m.clone()));
                                        let res = self.process_respsonse(m.clone(),packet.session_id,packet.clone().routing_header.hops[0]);
                                    },
                                    None => {
                                        println!("No message reconstructed yet");
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
            }
        }
    }

    
    fn recv_flood_response_n_handle(
        &mut self,
        session_id: u64,
        packet: &mut Packet,
        flood_packet: FloodResponse,
    ) -> Result<(()),&str>{
        if packet.routing_header.hops[packet.routing_header.hop_index] == self.id {
            self.client_topology
                .update_topology((self.id, NodeType::Client), flood_packet.path_trace.clone());
            println!("Path trace in client {:?}",flood_packet.path_trace.clone());
            let serv = self.client_topology.get_all_servers();
                if !serv.is_empty() {
                    for s in serv {
                        self.client_topology.find_all_paths(self.id, s);
                        self.send_new_server_req(s).ok();
                    }
                }
            println!("{:?}",self.client_topology.clone());
            return Ok(());
        } else {
            println!("{:?}",packet.routing_header);
            packet.routing_header.hop_index+=1;
            println!("{:?}",packet.routing_header);
            return self.send_flood_response(packet.session_id, &mut packet.clone());
        }
    }

    fn recv_flood_request_n_handle(
        &mut self,
        session_id: u64,
        packet: Packet,
        flood_packet: &mut FloodRequest,
    ) -> Result<(()),&str> {
        let mut path_trace  =  flood_packet.path_trace.clone();
        path_trace.push((self.id,NodeType::Server));
        if self.flood_ids.contains(&flood_packet.flood_id) {

            let mut hops = path_trace.clone().into_iter().map(|(id,_)|id).collect::<Vec<u8>>();
            hops.reverse();
            let flood_response = FloodResponse{
                flood_id: flood_packet.flood_id,
                path_trace: path_trace.clone()
            };
            let new_packet = Packet::new_flood_response(SourceRoutingHeader::with_first_hop(hops.clone()), session_id,flood_response.clone());
            return self.send_flood_response(session_id, &mut new_packet.clone());
        } else {
            self.flood_ids.insert(flood_packet.flood_id);
            for neigbor in self.packet_send.clone() {
                if neigbor.0 != path_trace.clone()[path_trace.clone().len()-2].0 {
                    let packet_new = Packet::new_flood_request(SourceRoutingHeader::empty_route(),
                    session_id, 
                    FloodRequest { 
                        flood_id: flood_packet.flood_id,
                        initiator_id: path_trace.clone()[0].0,
                        path_trace: path_trace.clone() }
                    );
                    self.packet_send.get(&neigbor.0).unwrap().send(packet_new.clone()).ok();
                    println!("Send to {}",neigbor.0);
                }
            }
            return Ok(());
        }
    }

    fn recv_nack_n_handle(&mut self, session_id: u64, nack: Nack, packet: &Packet) ->Result<(()),&str> {
        let flood_id = generate_flood_id(&mut self.flood_ids);
        let mut session = 0;
        while !self.session_id_alredy_used(session) {
            session = rand_session_id();
        }
        self.send_new_flood_request(session, flood_id);
        match nack.clone().nack_type {
            NackType::DestinationIsDrone => {
                //check route, it shouldn't happen if the routing was done right

                if let Some(packets) =  
                {
                    self.holder_sent.get(&(session_id,self.id)).cloned()
                }
                    {
                    for p in packets.clone() {
                        match p.clone().pack_type {
                            PacketType::MsgFragment(f) => {
                                if f.fragment_index == nack.fragment_index {
                                    self.client_topology.increment_weights_for_node(packet.routing_header.hops[0]);
                                    self.client_topology.update_current_path();
                                    self.send_new_generic_fragment(*p.routing_header.hops.last().unwrap(), session_id, f.clone()).unwrap();
                                }
                            },
                            PacketType::Ack(a)=>{
                                if a.fragment_index == nack.fragment_index {
                                    self.client_topology.increment_weights_for_node(packet.routing_header.hops[0]);
                                    self.client_topology.update_current_path();
                                    return self.send_ack(session_id, p.routing_header.hops.last().unwrap(), a.fragment_index);
                                }
                            },
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
                if let Some(fr) = 
                {
                    self.holder_sent.get(&(session_id,self.id))
                }
                {
                    for p in fr.clone() {
                        match p.clone().pack_type {
                            PacketType::MsgFragment(f)=>{
                                if f.fragment_index == nack.fragment_index {
                                    self.client_topology.increment_weights_for_node(packet.routing_header.hops[0]);
                                    self.client_topology.update_current_path();
                                    self.send_new_generic_fragment(*p.routing_header.hops.last().unwrap(), session_id, f.clone()).unwrap();
                                }
                            },
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
                if let Some(packets) = 
                {
                    self.holder_sent.get(&(session_id,self.id)).cloned()
                }
                {
                //update the path since it might mean a drone has crashed or bad routing
                    self.client_topology.increment_weights_for_node(id);
                    self.client_topology.update_current_path();
                    for p in packets.clone() {
                        match p.clone().pack_type {
                            PacketType::MsgFragment(f) => {
                                if f.fragment_index == nack.fragment_index {
                                    self.send_new_generic_fragment(*p.routing_header.hops.last().unwrap(), session_id, f.clone()).unwrap();
                                }
                            },
                            PacketType::Ack(a)=>{
                                if a.fragment_index == nack.fragment_index {
                                    return self.send_ack(session_id, p.routing_header.hops.last().unwrap(), a.fragment_index);
                                }
                            },
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
                if let Some(packets) = 
                {
                    self.holder_sent.get(&(session_id,self.id))
                }
                {
                    for p in packets.clone() {
                        match p.clone().pack_type {
                            PacketType::MsgFragment(f) => {
                                if f.fragment_index == nack.fragment_index {
                                    self.client_topology.increment_weights_for_node(id);
                                    self.client_topology.update_current_path();
                                    self.send_new_generic_fragment(*p.routing_header.hops.last().unwrap(), session_id, f.clone()).unwrap();
                                }
                            },
                            PacketType::Ack(a)=>{
                                if a.fragment_index == nack.fragment_index {
                                    self.client_topology.increment_weights_for_node(id);
                                    self.client_topology.update_current_path();
                                    return self.send_ack(session_id, p.routing_header.hops.last().unwrap(), a.fragment_index);
                                }
                            },
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

    fn recv_ack_n_handle(&mut self, src: NodeId, session_id: u64 , fragment_index: u64) -> Result<(()),&str> {
            if let Some(holder) =
            {
                self.holder_sent.get(&(session_id,self.id))
            }
            {
                if holder.is_empty() && fragment_index == 0{
                    return Err("All fragments of corrisponding message have been received");
                } else if holder.is_empty()&&fragment_index!=0{
                    return Err("Not supposed to receive this ACK");
                } else if !holder.is_empty() && fragment_index!=0{
                    let mut i = 0;
                    for f in holder.clone() {
                        match f.pack_type{
                            PacketType::Ack(a)=> {
                                if a.fragment_index==fragment_index {
                                    break;
                                }
                                i+=1;
                            },
                            _ => {}
                        }
                    }
    
                    self.holder_sent.get_mut(&(session_id,self.id)).unwrap().remove(i);
                    return Ok(())
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
    ) -> Option<Message>{
        self.client_topology.find_all_paths(self.id,src);
        self.client_topology.set_path_based_on_dst(src);
        self.send_ack(session_id, &src, frag.fragment_index).ok();
        if let Some(holder) = 
            self.holder_frag_index.get_mut(&(session_id,self.id))
        {
            if !holder.contains(&frag.fragment_index) {
                println!("Fragm n: 1  < n <  tot");
                let mut target = self.holder_rec.get_mut(&(session_id,src)).unwrap();
                update_holder_rec( target,&frag.data,  frag.length as usize, (session_id, src), frag.fragment_index as usize);
                holder.push(frag.fragment_index);
        
                print!("{} {}\n\n\n", holder.len(),frag.total_n_fragments);
                if holder.len() == (frag.total_n_fragments) as usize {
                    if let Some(mut data) = self.holder_rec.get_mut(&(session_id, src)) {
                        remove_trailing_zeros(&mut data);
                        let mut f_serialized = serialize(data.clone());
                        let mut result = fragmentation_handling::reconstruct_message(
                        data[0],
                        &mut f_serialized
                        );
                    
                        if let Ok(msg) = result {
                            self.holder_rec.remove(&(session_id, src));
                            self.holder_frag_index.remove(&(session_id, src));
                            self.pre_processed= Some(((session_id,src),msg.clone()));
                            println!("Message Reconstructed");
                            return Some(msg.clone());
                        } else {
                            self.pre_processed = None;
                            println!("Message reconstruction failed");
                            return None;
                        }
                    }
                }
            }
            None
        } else {
            self.holder_rec.insert(
                (session_id, src),
                vec![0; ((frag.clone().total_n_fragments * 128) )as usize],
            );
            println!("Firsr frag received");
            update_holder_rec(&mut self.holder_rec.get_mut(&(session_id,src)).unwrap(),&frag.data,  frag.length as usize, (session_id, src), frag.fragment_index as usize);
            self.holder_frag_index.insert((session_id,src),[frag.fragment_index].to_vec());
            return None;
        }
    }   


    fn session_id_alredy_used(&self,session_id: u64)->bool {
        if self.holder_sent.contains_key(&(session_id,self.id)){
            true
        } else {
            false
        }
    }
    
    fn get_hops(&mut self, dst: u8)->Option<Vec<u8>> {
        self.client_topology.find_all_paths(self.id,dst);
        self.client_topology.set_path_based_on_dst(dst);
        self.client_topology.get_current_path()
    }

}


fn rand_session_id()->u64{
    rand::random::<u64>()
}

fn fragment_packetization(fragments: &mut Vec<Fragment>, hops: Option<Vec<u8>>, session_id: u64)->Vec<Packet> {
    let mut vec = Vec::new();
    fragments.sort_by_key(|f| f.fragment_index);

    if hops.is_some(){
        for f in fragments {
            let packet = Packet::new_fragment(SourceRoutingHeader::with_first_hop(hops.clone().unwrap()), session_id, f.clone());
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


fn generate_flood_id(flood_ids: &mut HashSet<u64>) -> u64 {
    if flood_ids.is_empty() {
        flood_ids.insert(1);
        1
    } else {
        let mut rng = 1;
        while !flood_ids.insert(rng) {
            rng = rand::random::<u64>();
        }
        rng
    }
}

fn update_holder_rec(target: &mut Vec<u8>,data: &[u8], length: usize, key: (u64,NodeId), index: usize) {
    let mut finish_pos = ((index-1) * 128)+ 1;

    // Handle special case for the first fragment
    if index == 1 {
        target[0] = data[0];
        
    } else {

        if length<128 {
            finish_pos-=(128-length);
        }

        // Copy the fragment data into the correct position in the target vector
        target[finish_pos-length..finish_pos]
            .copy_from_slice(&data[..length]);
    }
}