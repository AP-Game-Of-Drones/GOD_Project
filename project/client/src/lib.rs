#![allow(dead_code,unused)]
use audio::AudioSource;
use codecs::png::PngDecoder;
use fragmentation_handling::DefaultsRequest;
use io::Reader;
use render::{render_resource::Extent3d, texture::ImageFormat};
use wg_2024::{packet::*,network::*};
use std::{collections::{HashMap,HashSet}, io::Cursor, mem::swap, ops::Deref, sync::Arc, thread, time::Duration};
use crossbeam_channel::*;
use controller::*;
use bevy::*;
use image::*;
use fragmentation_handling::*;
use topology::*;



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
///     -send message:
///         - defaults as the getter
///             enum Defaults {
///                 GETALLTEXT,
///                 GETALLMEDIALINKS?,
///                 REGISTER,
///                 SETUNAVAILABLE,
///                 SETAVAILABLE,
///                 GETALLAVAILABLE,
///             }
///         - complex for chatting
///             String input:
///                 fn send_chat_message(&self,String);
///                 fn receive_in_chat(&self)->String;
///     -establish connection to one sepcific client ( so a getter? 
///         does it envolves setting unavailability for these 2 clients?)
///     -getter for query res for serch on content server
///         fn get_web_item::<T>(&self,identifier: String)->T;
///     -delete itself from registered clients
///         fn delete_chat_account(&self);
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
    ChatClient,
    WebBrowser
}
// Client structure representing a client node in the network
#[derive(Clone)]
pub struct Client {
    pub id: NodeId, // Unique identifier for the client
    pub controller_send: Sender<NodeEvent>, // Sender for communication with the controller
    pub controller_recv: Receiver<NodeCommand>, // Receiver for commands from the controller
    pub packet_recv: Receiver<Packet>, // Receiver for incoming packets
    pub packet_send: HashMap<NodeId, Sender<Packet>>, // Map of packet senders for neighbors
    pub flood_ids: HashSet<u64>, // Set to track flood IDs for deduplication
    pub client_topology: topology::Topology,
    pub client_type: ClientType,
}


impl Client {

    pub fn run(&mut self) {
            loop {
                select_biased! {
                    recv(self.controller_recv) -> command_res => {
                        if let Ok(command) = command_res {
                            match command {
                                NodeCommand::SendPacket(p)=>{
                                    self.packet_send.get(&p.routing_header.hops[p.routing_header.hop_index]).unwrap().send(p.clone());
                                }
                            }
                        }
                    },
                    recv(self.packet_recv) -> packet_res => {
                        println!("Received:");
                        println!();
                        thread::sleep(Duration::from_secs(1));
                        if let Ok(packet) = packet_res {
                            match packet.clone().pack_type {
                                PacketType::FloodResponse(mut f)=>{
                                    if f.path_trace.clone()[0].0!=self.id{
                                        f.path_trace.reverse();
                                        f.path_trace.push((self.id,NodeType::Client));
                                        f.path_trace.reverse();
                                        println!("Yeee {:?}",f.path_trace);
                                    } else {
                                        println!("Fuck");
                                        println!("Yee {:?}",f.path_trace);
                                    }
                                },
                                PacketType::Nack(n)=>{
                                    println!("{:?} to Client {}",n,self.id);
                                },
                                PacketType::Ack(_)=>{println!("Ack")},
                                PacketType::FloodRequest(mut f)=>{
                                    f.path_trace.push((self.id,NodeType::Server));
                                    if let Some(_id) = self.flood_ids.get(&f.flood_id){
                                        let mut packet_t: Packet = Packet {
                                            pack_type: PacketType::FloodResponse(FloodResponse{
                                                flood_id: f.flood_id,
                                                path_trace: f.path_trace.clone(),
                                            }),
                                            routing_header: SourceRoutingHeader{
                                                hop_index: 1,
                                                hops: f.path_trace.clone().into_iter().map(|f| f.0).collect::<Vec<u8>>(),
                                            },
                                            session_id: packet.session_id,
                                        };
                                        packet_t.routing_header.hops.reverse();
                                        if let Some(destination) = packet_t.routing_header.hops.last() {
                                            if *destination != f.initiator_id {
                                                packet_t.routing_header.hops.push(f.initiator_id);
                                            }
                                        }
                                        let next_hop = packet_t.clone().routing_header.hops[packet_t.clone().routing_header.hop_index];
                                        self.packet_send.get(&next_hop).unwrap().send(packet_t.clone());
                                    }
                                    else {
                                        // f.path_trace.push((self.id,NodeType::Server));
                                        self.flood_ids.insert(f.flood_id);
                                        if self.packet_send.clone().len() > 1 {
                                            let prev_hop = f.path_trace[f.path_trace.len()-2].0;
                                            for send_to in self.packet_send.clone().into_iter(){
                                                let sub ;
                                                if f.path_trace[0].0 != f.initiator_id
                                                    && f.path_trace.clone().len() < 2
                                                {
                                                    sub = 1;
                                                } else {
                                                    sub = 2;
                                                }
                                                if send_to.0
                                                != f.path_trace.clone()[f.path_trace.clone().len() - sub].0{
                                                    let packet_r = Packet {
                                                        pack_type: PacketType::FloodRequest(FloodRequest{
                                                            initiator_id: f.initiator_id,
                                                            flood_id: f.flood_id,
                                                            path_trace: f.path_trace.clone(),
                                                        }),
                                                        routing_header: SourceRoutingHeader {
                                                            hop_index: 1,
                                                            hops: [].to_vec(),
                                                        },
                                                        session_id:packet.clone().session_id,
                                                    };
    
                                                    send_to.1.send(packet_r.clone()).ok();
                                                }
                                            }
                                        }
                                        else {
                                            let mut packet_t: Packet = Packet {
                                            pack_type: PacketType::FloodResponse(FloodResponse{
                                                flood_id: f.flood_id,
                                                path_trace: f.path_trace.clone(),
                                            }),
                                            routing_header: SourceRoutingHeader{
                                                hop_index: 1,
                                                hops: f.path_trace.clone().into_iter().map(|f| f.0).collect::<Vec<u8>>(),
                                            },
                                            session_id: packet.session_id,
                                            };
                                            packet_t.routing_header.hops.reverse();
                                            if let Some(destination) = packet_t.routing_header.hops.last() {
                                                if *destination != f.initiator_id {
                                                    packet_t.routing_header.hops.push(f.initiator_id);
                                                }
                                            }
                                            let next_hop = packet_t.clone().routing_header.hops[packet_t.clone().routing_header.hop_index];
    
                                            println!("{:?} {} {}",packet_t.routing_header.clone(),self.id, next_hop);
    
                                            self.packet_send.get(&next_hop).unwrap().send(packet_t.clone());
                                        }
                                    }
                                },
                                _=>{}
                                }
                        }
                    },
                }
            }
        }
    
    fn send_flood_request(&self, session_id: u64, flood_id: u64)->Result<(()),&str> {
        for neighbors in self.packet_send.clone(){
            if let Err(res) = self.packet_send.get(&neighbors.0).unwrap().send(Packet::new_flood_request(SourceRoutingHeader::empty_route(), session_id, FloodRequest::new(flood_id,self.id))){
                return Err("One or more neighbors was not found");
            } 
        }
        Ok(())
    }

    fn send_flood_response(&self, session_id: u64, flood_id: u64, path_trace: Vec<(NodeId,NodeType)>)->Result<(()),&str>{
        for neighbors in self.packet_send.clone(){
            if neighbors.0!=path_trace.clone()[path_trace.clone().len()-2].0{
                if let Err(res) = self.packet_send.get(&neighbors.0).unwrap().send(Packet::new_flood_response(SourceRoutingHeader::with_first_hop(path_trace.clone().into_iter().map(|f|f.0).rev().collect::<Vec<u8>>()), session_id, FloodResponse { flood_id, path_trace: path_trace.clone() })){
                    return Err("One or more sender was corrupted");
                }
            }
        }
        Ok(())
    }

    fn send_ack(&self, session_id: u64, first_hop: &u8, hops: Vec<u8>, fragment_index: u64)->Result<(()),&str>{
        if let Some(sender) = self.packet_send.get(first_hop){
            if let Err(e) = sender.send(Packet::new_ack(SourceRoutingHeader::with_first_hop(hops), session_id, fragment_index)){
                return Err("Sender error");
            }else {
                return Ok(());
            }
        } else {
            return Err("No sender found");
        }
    }

    fn send_default_request(&self, server_id: NodeId, session_id: u64, request: DefaultsRequest)->Result<(()),&str>{
        let paths = self.client_topology.shortest_path(self.id, server_id);
        let bytes = <DefaultsRequest as fragmentation_handling::Fragmentation::<DefaultsRequest>>::fragment(request);
        let fragments = fragmentation_handling::serialize(bytes);
        let mut packets = Vec::new();
        
        if let Some(trace) = paths {
            for fr in fragments {
                packets.push(Packet::new_fragment(SourceRoutingHeader::with_first_hop(trace.clone()), session_id, fr));
            }
            if trace[0]==self.id {
                for packet in packets {
                    self.packet_send.get(&trace[1]).unwrap().send(packet.clone()).expect("Sender error");
                }
                Ok(())
            } else {
                for mut packet in packets {
                    let mut vec = [self.id].to_vec();
                    packet.routing_header.hops.append(&mut vec);
                    packet.routing_header.hops = vec.clone();
                    self.packet_send.get(&trace[1]).unwrap().send(packet.clone()).expect("Sender error");
                }
                Ok(())
            }
        } else {
            Err("Error in source routing")
        }
    }

    fn send_string_query(&self, server_id: NodeId, session_id: u64, query: String)->Result<(()),&str>{
        let paths = self.client_topology.shortest_path(self.id, server_id);
        let bytes = <String as fragmentation_handling::Fragmentation::<String>>::fragment(query);
        let fragments = fragmentation_handling::serialize(bytes);
        let mut packets = Vec::new();
        
        if let Some(trace) = paths {
            for fr in fragments {
                packets.push(Packet::new_fragment(SourceRoutingHeader::with_first_hop(trace.clone()), session_id, fr));
            }
            if trace[0]==self.id {
                for packet in packets {
                    self.packet_send.get(&trace[1]).unwrap().send(packet.clone()).expect("Sender error");
                }
                Ok(())
            } else {
                for mut packet in packets {
                    let mut vec = [self.id].to_vec();
                    packet.routing_header.hops.append(&mut vec);
                    packet.routing_header.hops = vec.clone();
                    self.packet_send.get(&trace[1]).unwrap().send(packet.clone()).expect("Sender error");
                }
                Ok(())
            }
        } else {
            Err("Error in source routing")
        }
    }

    fn send_generic_fragment(&self, server_id: NodeId, session_id: u64, fragment: Fragment)->Result<(()),&str>{
        if let Some(trace) = self.client_topology.shortest_path(self.id, server_id) {
            if let Some(sender) = self.packet_send.get(&trace[0]) {
                if let Ok(_) = sender.send(Packet::new_fragment(SourceRoutingHeader::with_first_hop(trace.clone()), session_id, fragment)){
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

}