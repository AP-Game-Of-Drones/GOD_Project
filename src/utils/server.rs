// // server/src/lib

//  // “message.rs” holds ChatRequest, ChatResponse, Message<...>, etc.


// use fragmentation_handling::ChatMessages;
// use fragmentation_handling::Message::{ChatRequest, ChatResponse, DefaultsRequest, DroneSend, Message, Request, Response};
// use crossbeam_channel::{Receiver, Sender, select_biased, RecvError};
// use std::collections::{HashMap, HashSet};
// use std::ptr::addr_of_mut;
// use controller::{NodeCommand, NodeEvent};
// use serde::Serialize;
// use wg_2024::network::{NodeId, SourceRoutingHeader};
// use wg_2024::packet::{Packet, PacketType, Fragment, Ack, Nack,
//                       FloodRequest, FloodResponse, NackType, NodeType};
// use topology::Topology;
// use wg_2024::controller::DroneEvent;
// use fragmentation_handling::{serialize, reconstruct_message, deconstruct_message, Fragmentation};
// use fragmentation_handling::Message::DefaultResponse;
// // 1. Flood-Based Topology Discovery
// //    [x] handle_flood_request:
// //         - Check if (flood_id, initiator_id) is seen via `self.flood_ids()`.
// //         - If seen: append (self.id(), NodeType::Server) to `path_trace`, construct FloodResponse, reverse `routing_header.hops`, send back along hops[1].
// //         - If not seen: append (self.id(), NodeType::Server) to `path_trace`, call `self.topology().update_topology((initiator_id, NodeType::Server), path_trace.clone())`, forward FloodRequest to all neighbors except the previous hop.
// //    [x] handle_flood_response:
// //         - Pull off the first element of `path_trace` and call `self.topology().update_topology((initiator_id, NodeType::Server), remaining_trace)`
// //         - Ensure this server’s type is marked via `set_node_type(self.id(), NodeType::Server)`
// //         - If not initiator: reverse `routing_header.hops`, increment `hop_index`, send FloodResponse toward initiator.
// //      (✔️ Done in `ChatServer::handle_flood_request` and `ChatServer::handle_flood_response`)

// // 2. Fragment-Level Message Plumbing
// //    [ ] In Server::run loop, on PacketType::MsgFragment(frag):
// //         - Verify `header.hops[hop_index] == self.id()`. If mismatch: send Nack(ErrorInRouting).
// //         - Increment `hop_index`.
// //         - If `hop_index == hops.len()`: buffer each fragment in `self.buffers()`, record `self.last_header().insert((session_id, source_id), original_header.clone())`. Once all fragments for (session_id, source_id) are received, call `reconstruct_message(...)` and then `on_request_arrived(source_id, session_id, json_string)`.
// //            · After full reassembly and high‐level handler, send back an `Ack(fragment_index)` for each fragment.
// //         - Else: check next_hop = `hops[hop_index]` is neighbor. If not, send Nack(ErrorInRouting). Otherwise, forward Packet { updated header, same session_id, MsgFragment } to `self.packet_send().get(&next_hop)`.
// //      (❌ Not implemented yet; `run` only handles Flood messages so far)

// // 3. Ack / Nack Handling
// //    [ ] In run loop, on PacketType::Ack(ack):
// //         - Lookup (session_id, source_id) in `self.buffers()` and remove acknowledged fragment (or mark delivered).
// //    [ ] On PacketType::Nack(nack):
// //         - If `nack_type == DestinationIsDrone`: drop.
// //         - Else: reverse-hop the Nack back using `self.last_header()` for (session_id, source_id). If any hop missing, send via Simulation Controller.
// //      (❌ Not implemented yet)

// // 4. ChatServer: High-Level Request Processing
// //    [x] handle_request(ChatRequest::ClientList):
// //         - Return `ChatResponse::ClientList(self.registered_clients.iter().cloned().collect())`.
// //    [x] handle_request(ChatRequest::Register(client_id)):
// //         - Insert `client_id` into `self.registered_clients`, return `ChatResponse::ClientList(...)`.
// //    [x] handle_request(ChatRequest::SendMessage { from, to, message }):
// //         - Use `self.topology()` to compute shortest path.
// //         - Wrap payload in `ChatMessages::CHATSTRING`, fragment via `<ChatMessages as Fragmentation<_>>::fragment`, then `serialize(...)` → Vec<Fragment>.
// //         - Send each fragment as Packet { header.clone(), new session_id, MsgFragment } to `self.packet_send().get(&path[1])`.
// //      (✔️ Done in `ChatServer::handle_request`)

// // 5. send_response(...) Implementation
// //    [x] Turn high‐level response into JSON with `response.stringify()`, then `raw_bytes = json.into_bytes()`.
// //    [x] Call `serialize(raw_bytes)` → Vec<Fragment>.
// //    [x] Generate `reply_session_id = self.next_session_id; self.next_session_id += 1`.
// //    [x] Lookup original `(orig_session, client_id) → header` in `self.last_header()`, remove it.
// //    [x] Reverse `orig_header.hops`, build `reply_header = SourceRoutingHeader { hops: reversed, hop_index: 0 }`.
// //    [x] Send each fragment as Packet { reply_header.clone(), reply_session_id, MsgFragment } to `self.packet_send().get(&reversed[1])`.
// //    [ ] Handle “no recorded header” or “reversed path too short” edge cases (currently logs an error).
// //      (✔️ Main logic done in `ChatServer::send_response`; error logging in place)

// // 6. ContentServer (Text + Media)
// //    [ ] Implement a `ContentServer` struct mirroring `ChatServer` but using `TextRequest`/`TextResponse` or `MediaRequest`/`MediaResponse`.
// //         - handle_request(TextRequest::TextList): read “assets/text/…”, return list.
// //         - handle_request(TextRequest::Text(id)): read file or return NotFound.
// //         - handle_request(MediaRequest::MediaList): read “assets/media/…”, return list.
// //         - handle_request(MediaRequest::Media(id)): read media bytes or return NotFound.
// //         - send_response(...) identical to ChatServer’s implementation.
// //      (❌ Not started yet)

// // 7. message.rs Types
// //    [x] ChatRequest, ChatResponse, TextRequest, TextResponse, MediaRequest, MediaResponse, DefaultsRequest, DefaultResponse, ContentRequest, ContentResponse all derive Serialize/Deserialize, impl DroneSend, impl Request/Response.
// //    [ ] Verify that “DefaultsRequest`/`DefaultResponse` and `ContentRequest`/`ContentResponse` match protocol spec exactly.
// //      (✔️ Basic enums in `server/src/message.rs`; full “Defaults” and “Content” not used until ContentServer)

// // 8. Topology Data Structure
// //    [x] `Topology::new()`, `add_node()`, `update_topology()`, `set_node_type()`, `find_all_paths()`, `update_current_path()`, `get_current_path()`, `get_all_servers()`, `get_neighbors()` implemented.
// //      (✔️ Done in `topology.rs`)

// // 9. Integrate with Network Initializer
// //    [ ] In `network_initializer.rs`, add thread spawns for `ChatServer::new(...)` (and later `ContentServer::new(...)`) with appropriate `packet_recv` and `packet_send` channels.
// //      (❌ Not yet wired in `initialize()`—only drones, clients, and unspawned SimulationController present)

// // 10. Smoke Tests & Validation
// //    [ ] Test FloodRequests on a minimal topology: verify `topology.nodes` is populated correctly.
// //    [ ] Test sending a ChatRequest from a mock “client” through a mock “drone” to ChatServer and back: verify reassembly and `ChatResponse` fragments are sent in reverse path.
// //    [ ] Test TextRequest/MediaRequest against a small asset directory for correct responses.
// //      (❌ No tests written yet)

// // 11. Documentation & Comments
// //    [x] Annotated server code with references to AP-protocol.md sections.
// //    [ ] Add comments linking each major function to its protocol spec location.
// //      (✔️ Partial comments present; could add more explicit AP-protocol.md references)






// pub enum ServerType{
//     Content,
//     Chat,
// }

// /// Any high‐level “request” (chat or content) must implement `Request`:

// pub trait Server {

//     //high‐level request enum
//     type RequestType: Request + DroneSend;

//     //high‐level response enum
//     type ResponseType: Response + DroneSend;


//     /// Return the NodeId of this server.
//     fn id(&self) -> NodeId;

//     /// Channel on which incoming `Packet`s (fragments) arrive from neighbor drones.
//     fn packet_recv(&self) -> &Receiver<Packet>;


//     /// How to send a `Packet` to each neighbor drone (map: neighbor_id → Sender<Packet>).
//     fn packet_send(&self) -> &HashMap<NodeId, crossbeam_channel::Sender<Packet>>;

//     // Our local network topology (populated via FloodResponse).
//     fn topology(&mut self) -> &mut Topology;

//     /// A buffer of fragments for each in‐flight session: `(session_id, src_id)` → `Vec<Fragment>`.
//     fn buffers(&mut self) -> &mut HashMap<(u64, NodeId), Vec<Fragment>>;

//     /// The last `SourceRoutingHeader` we saw per `(session_id, src_id)`, so we know how to reply.
//     fn last_header(&mut self) -> &mut HashMap<(u64, NodeId), SourceRoutingHeader>;

//     // Which `(flood_id, initiator_id)` pairs we’ve already processed (so we don’t re-flood).
//     fn flood_ids(&mut self) -> &mut HashSet<(u64, NodeId)>;


//     fn compose_message(
//         source_id: NodeId,
//         session_id: u64,
//         raw_content: String,
//     ) -> Result<Message<Self::RequestType>, String> {
//         let content = Self::RequestType::from_string(raw_content)?;
//         Ok(Message {
//             session_id,
//             source_id,
//             content,
//         })
//     }

//     fn on_request_arrived(&mut self, source_id: NodeId, session_id: u64, raw_content: String) {
//         if let Ok(default_req)= DefaultsRequest::from_string(raw_content.clone()){
//             match default_req{
//                 DefaultsRequest::GETSERVERTYPE=>{
//                     // let resp= DefaultResponse::S
//                     // self.send_default_response(resp, session_id, source_id);
//                     // return;
//                 }
//                 _ => {
//                     // if you want to handle other DefaultsRequest variants here, do so…
//                     // otherwise fall through to the ChatRequest logic below}
//                 }
                
//             }
//         }
        
//         match Self::compose_message(source_id, session_id, raw_content) {
//             Ok(message) => {
//                 let response = self.handle_request(message.content);
//                 self.send_response(response, session_id, source_id);
//             }
//             Err(str) => panic!("{}", str),
//         }
//     }

//     fn send_response(&mut self, _response: Self::ResponseType, _orig_session: u64, _client_id: NodeId) {
//         // send response
//     }

//     fn handle_request(&mut self, request: Self::RequestType) -> Self::ResponseType;

//     fn get_sever_type() -> ServerType;


//     fn new(
//         id: NodeId,
//         controller_recv: Receiver<NodeCommand>, //TBD (CONFIRM WITH STEFANO)
//         packet_recv: Receiver<Packet>,
//         packet_send: HashMap<NodeId, Sender<Packet>>,
//         sc_sender: Sender<DroneEvent>,
//     )-> Self;

//     fn run(&mut self);

//     //if unseen, learn topology and forward; otherwise send FloodResponse back.
//     fn handle_flood_request(&mut self, mut fq: FloodRequest, header: &SourceRoutingHeader) {
//         //ensure initiator is the very first element
//         if fq.path_trace.is_empty() {
//             fq.path_trace.push((fq.initiator_id, NodeType::Server));
//         }
//         else {
//             let first = fq.path_trace[0];
//             if first.0 != fq.initiator_id || first.1 != NodeType::Server {
//                 fq.path_trace.insert(0, (fq.initiator_id, NodeType::Server));
//             }
//         }

//         let key = (fq.flood_id, fq.initiator_id);

//         // First, check “seen” and return early if we’ve already processed this flood.
//         // We wrap this in its own block so that the mutable borrow of `self` ends immediately.
//         let already_seen: bool = {
//             let flood_ids = self.flood_ids(); // ← mutable borrow of `self`
//             flood_ids.contains(&key)
//         }; // ← that borrow ends here

//         if already_seen {
//             //add itself to path_trace
//             fq.path_trace.push((self.id(), NodeType::Server));

//             //create a flood response
//             let resp = FloodResponse {
//                 flood_id: fq.flood_id,
//                 path_trace: fq.path_trace.clone(),
//             };

//             //Calculate reverse path and create source routing header to send flood response back
//             let mut rev_hops: Vec<NodeId> = fq
//                 .path_trace
//                 .iter()
//                 .map(|(node_id, _node_type)| *node_id)
//                 .collect();
//             rev_hops.reverse();

//             let response_header = SourceRoutingHeader {
//                 hops: rev_hops,
//                 hop_index: 1,
//             };

//             //create a packet of floodResponse and send it back
//             let pkt = Packet {
//                 routing_header: response_header,
//                 session_id: 0, //CONFIRM whether it should be zero
//                 pack_type: PacketType::FloodResponse(resp),
//             };

//             // 1) check next hop exists or not
//             // 2) if yes, check sender channel of that hop
//             // 3) send the pkt
//             if let Some(&next) = pkt.routing_header.hops.get(1) {
//                 if let Some(sender_channel) = self.packet_send().get(&next) {
//                     let _ = sender_channel.send(pkt);
//                 }
//             }
//             return;
//         }

//         //not seen
//         else {
//             //mark it as seen
//             self.flood_ids().insert(key);

//             //add itself to path_trace
//             fq.path_trace.push((self.id(), NodeType::Server));

//             //update local topology with new connections
//             self.topology()
//                 .update_topology((fq.initiator_id, NodeType::Server), fq.path_trace.clone());

//             // forward flood request to each neighbor except previous one
//             let prev_hop =
//                 //CHECK: ensure hop index is proper
//                 if header.hop_index < header.hops.len() {
//                     let prev_hop_index = header.hop_index;
//                     header.hops[prev_hop_index]
//                 } else {
//                     0 // hop_index out of range.dummy NodeID
//                 };

//             //gather all neighbors except prev_hop
//             let mut forward_neighbors = Vec::new();
//             for &nbr in self.packet_send().keys() {
//                 if nbr != prev_hop {
//                     forward_neighbors.push(nbr);
//                 }
//             }

//             //if no neighbors, send FloodResponse
//             if forward_neighbors.is_empty() {
//                 let resp = FloodResponse {
//                     flood_id: fq.flood_id,
//                     path_trace: fq.path_trace.clone(),
//                 };

//                 //rev_hops for sending it back
//                 let mut rev_hops: Vec<NodeId> = fq
//                     .path_trace
//                     .iter()
//                     .map(|(node_id, _node_type)| *node_id)
//                     .collect();
//                 rev_hops.reverse();

//                 let response_header = SourceRoutingHeader {
//                     hops: rev_hops.clone(),
//                     hop_index: 1,
//                 };

//                 let pkt = Packet {
//                     routing_header: response_header,
//                     session_id: 0,
//                     pack_type: PacketType::FloodResponse(resp),
//                 };

//                 if let Some(&next) = rev_hops.get(1) {
//                     if let Some(chan) = self.packet_send().get(&next) {
//                         let _ = chan.send(pkt);
//                     }
//                 }
//                 return;
//             }

//             // Otherwise if neighbors present
//             for &nbr in &forward_neighbors {
//                 let forward_pkt = Packet {
//                     routing_header: SourceRoutingHeader {
//                         hop_index: header.hop_index,
//                         hops: header.hops.clone(),
//                     },
//                     session_id: 0,
//                     pack_type: PacketType::FloodRequest(fq.clone()),
//                 };

//                 if let Some(chan) = self.packet_send().get(&nbr) {
//                     let _ = chan.send(forward_pkt);
//                 }
//             }
//         }
//     }


//     fn handle_flood_response(&mut self, fr: FloodResponse, header: &SourceRoutingHeader){
//         // merge entire path_trace into our topology

//         let mut trace= fr.path_trace.clone();
//         let first= trace.remove(0); // (initiator_id, initiator_type)
//         self.topology().update_topology(first, trace);

//         //ensure this server is marked as server in the topology
//         let self_id =self.id();

//         // finish if we are the initiator
//         let initiator_id = first.0;
//         if self.id() == initiator_id { return; }

//         //send it back to initiator
//         let next_index = header.hop_index+1;
//         if let Some(&next_hop) = header.hops.get(next_index){
//             let response_header= SourceRoutingHeader{
//                 hops: header.hops.clone(),
//                 hop_index: next_index,
//             };

//             let pkt=Packet{
//                 routing_header: response_header,
//                 session_id: 0,
//                 pack_type: PacketType::FloodResponse(fr),
//             };

//             if let Some(sender_channel)= self.packet_send().get(&next_hop){
//                 let _ = sender_channel.send(pkt);
//             }
//         }

//     }



// }



// pub struct ChatServer{
//     id: NodeId,
//     controller_recv: Receiver<NodeCommand>, //A Channel from the SC  //CONFIRM STEFANO WHETHER SERVER WILL RECEIVE COMMANDS FROM THE CONTROLLER
//     packet_recv: Receiver<Packet>, //A channel on which drone packet will arrive
//     packet_send: HashMap<NodeId, Sender<Packet>>, //HashMap of each neighbour's NodeId to the Sender<Packet> for that neighbour
//     flood_ids: HashSet<(u64, NodeId)>, // to remember (flood_id, initiator) already seen, so we dont reflood forever.
//     topology: Topology, //a DS when fully flooded, contains all nodes. also can give SR paths
//     registered_clients: HashSet<NodeId>,
//     buffers: HashMap<(u64, NodeId), Vec<Fragment>>,
//     last_header: HashMap<(u64, NodeId), SourceRoutingHeader>,
//     next_session_id: u64,
//     sc_sender: Sender<DroneEvent>
    
// }

// impl Server for ChatServer {
//     type RequestType = ChatRequest;
//     type ResponseType = ChatResponse;

//     fn id(&self) -> NodeId {
//         self.id
//     }

//     fn packet_recv(&self) -> &Receiver<Packet> {
//         &self.packet_recv
//     }

//     fn packet_send(&self) -> &HashMap<NodeId, Sender<Packet>> {
//         &self.packet_send
//     }

//     fn topology(&mut self) -> &mut Topology {
//         &mut self.topology
//     }

//     fn buffers(&mut self) -> &mut HashMap<(u64, NodeId), Vec<Fragment>> {
//         &mut self.buffers
//     }

//     fn last_header(&mut self) -> &mut HashMap<(u64, NodeId), SourceRoutingHeader> {
//         &mut self.last_header
//     }

//     fn flood_ids(&mut self) -> &mut HashSet<(u64, NodeId)> {
//         &mut self.flood_ids
//     }

//     fn handle_request(&mut self, request: Self::RequestType) -> Self::ResponseType {
//         match request {
//             ChatRequest::ClientList => {
//                 //return client list
//                 println!("Sending ClientList");
//                 let client_list = self.registered_clients.iter().cloned().collect();
//                 ChatResponse::ClientList(client_list)
//             }
//             ChatRequest::Register(client_id) => {
//                 //register new client
//                 println!("Registering client ID = {}", client_id);
//                 self.registered_clients.insert(client_id);

//                 //return client list
//                 let clients: Vec<NodeId> = self.registered_clients.iter().cloned().collect();
//                 ChatResponse::ClientList(clients)
//             }
//             ChatRequest::SendMessage { from, to, message } => {
//                 println!("Forwarding message \"{}\" from {} → {}", message, from, to);
//                 //create a new session id for this new chat-message
//                 let session_id = self.next_session_id;
//                 self.next_session_id += 1;

//                 //use topology to get all paths and pick the shortest one
//                 let self_id = self.id;
//                 self.topology().find_all_paths(self_id, to);
//                 self.topology().update_current_path();
//                 let u8_path = self.topology().get_current_path(); // Vec of NodeId but as u8
//                 let path: Vec<NodeId> = u8_path.iter().map(|&node_id| node_id as NodeId).collect(); //Vec of NodeID

//                 //if empty path -> Error
//                 if path.len() < 2 || path[0] != self.id || path[path.len() - 1] != to {
//                     eprintln!("ERROR: invalid or missing path from {} → {}: {:?}",
//                               self.id(),
//                               to,
//                               path
//                     );
//                     return ChatResponse::MessageSent; // or an error variant
//                 }

//                 // create source-routing header
//                 let header = SourceRoutingHeader {
//                     hops: path.clone(),
//                     hop_index: 0,
//                 };

//                 //wrap chat request into message and fragment it
//                 let chat_payload = ChatMessages::CHATSTRING(from, to, message.clone());
//                 let raw_bytes = <ChatMessages as Fragmentation<ChatMessages>>::fragment(chat_payload);
//                 let fragments: Vec<Fragment> = serialize(raw_bytes);

//                 //for each fragment,build a Packet type and send to hops[1]
//                 if let Some(&first_hop) = header.hops.get(1) {
//                     for frag in fragments.into_iter() {
//                         let pkt = Packet {
//                             routing_header: header.clone(),
//                             session_id,
//                             pack_type: PacketType::MsgFragment(frag),
//                         };

//                         if let Some(chan) = self.packet_send.get(&first_hop) {
//                             let _ = chan.send(pkt);
//                         } else {
//                             eprintln!("ERROR: no channel for next hop {} (full path = {:?})", first_hop, header.hops);
//                         }
//                     }
//                 } else {
//                     // This can only happen if `path.len() < 2`, which we already checked above.
//                     eprintln!("ERROR: computed path too short: {:?}", path);
//                 }

//                 //return MessageSent
//                 ChatResponse::MessageSent
//             }
//         }
//     }

//     fn get_sever_type() -> ServerType {
//         ServerType::Chat
//     }

//     fn new(id: NodeId,controller_recv: Receiver<NodeCommand> , packet_recv: Receiver<Packet>, packet_send: HashMap<NodeId, Sender<Packet>>, sc_sender: Sender<DroneEvent>) -> Self {
//         ChatServer {
//             id,
//             controller_recv,
//             packet_recv,
//             packet_send,
//             flood_ids: HashSet::new(),
//             topology: Topology::new(),
//             registered_clients: HashSet::new(),
//             buffers: HashMap::new(),
//             last_header: HashMap::new(),
//             next_session_id: 1,
//             sc_sender,
//         }
//     }

//     fn send_response(&mut self, response: Self::ResponseType, orig_session: u64, client_id: NodeId) {
//         // Step 1: Serialize the high‐level response into bytes (JSON).
//         let json = response.stringify();
//         let raw_bytes = json.into_bytes();

//         // Step 2: Split those bytes into 128‐byte Fragments.
//         let data_frags: Vec<Fragment> = serialize(raw_bytes);

//         // Step 3: Allocate a fresh session ID for this reply.
//         let reply_session_id = self.next_session_id;
//         self.next_session_id += 1;

//         // Step 4: Look up exactly the header that was stored under (orig_session, client_id).
//         if let Some(orig_header) = self.last_header.get(&(orig_session, client_id)) {
//             // Step 5: Build the reversed hop‐vector:
//             let mut rev_hops = orig_header.hops.clone();
//             rev_hops.reverse();

//             // Step 6: The reply’s routing header starts at index 0 on that reversed path.
//             let reply_header = SourceRoutingHeader {
//                 hops: rev_hops.clone(),
//                 hop_index: 0,
//             };

//             // Step 7: If there is at least one “real” next‐hop (rev_hops[1]), send each fragment there.
//             if rev_hops.len() >= 2 {
//                 let next_hop = rev_hops[1];
//                 for frag in data_frags.into_iter() {
//                     let pkt = Packet {
//                         routing_header: reply_header.clone(),
//                         session_id: reply_session_id,
//                         pack_type: PacketType::MsgFragment(frag),
//                     };

//                     if let Some(chan) = self.packet_send.get(&next_hop) {
//                         let _ = chan.send(pkt);
//                     } else {
//                         // If we don’t have a direct channel to that neighbor, ask the SC
//                         // to “shortcut” this packet instead.
//                         let _ = self.sc_sender.send(DroneEvent::ControllerShortcut(pkt));
//                     }
//                 }
//             } else {
//                 eprintln!("ERROR: reversed path too short: {:?}", rev_hops);
//             }

//             // Step 8: Finally, remove that header entry so we don’t reuse it.
//             self.last_header.remove(&(orig_session, client_id));
//         } else {
//             eprintln!(
//                 "ERROR: no recorded header for outgoing response (session={}, client={})",
//                 orig_session, client_id
//             );
//         }
//     }

//     fn run(&mut self) {
//         println!("ChatServer[{}] starting run()", self.id());

//         loop {
//             select_biased! {
//                 //first listen to controller //CONFIRM WITH STEFANO
//                 recv(self.controller_recv)-> cmd_res => match cmd_res{
//                     Ok(cmd) => {
//                         match cmd{
//                             NodeCommand::AddSender(new_id, new_chan)=>{
//                                 self.packet_send.insert(new_id, new_chan);
//                             }

//                             NodeCommand::RemoveSender(node_id)=>{
//                                 self.packet_send.remove(node_id);
//                             }

//                             NodeCommand::SetPacketDropRate(_pdr)=>{
//                                 //IGNORE: server doesn't have pdr
//                             }

//                             NodeCommand::Crash => {
//                                 println!("ChatServer[{}] received Crash → shutting down"
//                                     , self.id());
//                                 return;
//                             }

//                         }


//                     }
//                     Err(_) => {
//                         // Controller channel closed; shut down
//                         println!("ChatServer[{}]: controller channel closed, exiting", self.id());
//                         return;
//                     }
//                 },
//                 // if no controller command is there, then process packet from packet_recv
//                 recv(self.packet_recv)->pkt_res => match pkt_res{

//                     Ok(pkt) => {self.handle_incoming_pkt(pkt)}
//                     Err(_) => {
//                         // packet_recv channel closed → exit
//                         println!("ChatServer[{}]: packet_recv closed, exiting", self.id());
//                         return;
//                     }
//                 },
//             }
//         }
//     }

// }

// impl ChatServer{
//     fn handle_incoming_pkt(&mut self, pkt: Packet){
//         match pkt.pack_type{
//             PacketType::MsgFragment(mut frag) => {
//                 // Clone the header so we can modify hop_index
//                 let mut header = pkt.routing_header.clone();
//                 let session_id = pkt.session_id;
//                 let src_id = header.hops[0]; // original sender ID

//                 // 1.a) Check “correct hop”:
//                 if header.hops[header.hop_index] != self.id() {
//                     // Wrong recipient → send low‐level NACK(ErrorInRouting)
//                     let err = Nack {
//                         fragment_index: frag.fragment_index,
//                         nack_type: NackType::ErrorInRouting(self.id()),
//                     };

//                     // Reverse the hop list to route the NACK back:
//                     let mut rev_hops = header.hops.clone();
//                     rev_hops.reverse();

//                     let reply_header = SourceRoutingHeader {
//                         hops: rev_hops.clone(),
//                         hop_index: 0,
//                     };

//                     let nack_pkt = Packet {
//                         routing_header: reply_header.clone(),
//                         session_id,
//                         pack_type: PacketType::Nack(err),
//                     };

//                     // Send NACK to rev_hops[1], if it exists
//                     if rev_hops.len() >= 2 {
//                         let next = rev_hops[1];
//                         if let Some(chan) = self.packet_send.get(&next) {
//                             let _ = chan.send(nack_pkt);
//                         } else {
//                             eprintln!(
//                                 "ERROR: no channel to send NACK to {} (rev_hops={:?})",
//                                 next, rev_hops
//                             );
//                         }
//                     } else {
//                         eprintln!("ERROR: cannot send NACK, reversed path too short: {:?}", rev_hops);
//                     }
//                     return;
//                 }

//                 // 1.b) We are the intended hop. Advance hop_index:
//                 header.hop_index += 1;

//                 // 1.c) If this server is now the *final* destination (hop_index == hops.len()):
//                 if header.hop_index == header.hops.len() {
//                     // • Record the original header so we can reverse‐route ACKs later:
//                     self.last_header().insert((session_id, src_id), pkt.routing_header.clone());

//                     // • Buffer this fragment:
//                     let entry = self.buffers()
//                         .entry((session_id, src_id))
//                         .or_insert_with(Vec::new);
//                     entry.push(frag.clone());

//                     // • Check if we have all fragments for this (session_id, src_id):
//                     let all_frags = &*entry;
//                     if all_frags.len() as u64 == frag.total_n_fragments {
//                         // Reassemble entire byte buffer:
//                         let mut raw_bytes = Vec::with_capacity((frag.total_n_fragments as usize) * 128);
//                         raw_bytes.resize((frag.total_n_fragments as usize) * 128, 0u8);

//                         for f in all_frags.iter() {
//                             let offset = (f.fragment_index as usize) * 128;
//                             raw_bytes[offset..offset + (f.length as usize)]
//                                 .copy_from_slice(&f.data[0..(f.length as usize)]);
//                         }

//                         // Trim trailing zero‐padding after the last fragment:
//                         let last_idx = frag.fragment_index as usize;
//                         let last_len = frag.length as usize;
//                         let true_len = last_idx * 128 + last_len;
//                         raw_bytes.truncate(true_len);

//                         // Convert bytes → String (JSON) and call on_request_arrived:
//                         if let Ok(json_str) = String::from_utf8(raw_bytes.clone()) {
//                             self.on_request_arrived(src_id, session_id, json_str);
//                         } else {
//                             eprintln!("ERROR: could not parse reassembled bytes as UTF-8");
//                         }

//                         // Now send back a low‐level ACK for each fragment index:
//                         let orig_header = self.last_header().get(&(session_id, src_id)).unwrap();
//                         let mut rev_hops = orig_header.hops.clone();
//                         rev_hops.reverse();
//                         let base_reply_header = SourceRoutingHeader {
//                             hops: rev_hops.clone(),
//                             hop_index: 0,
//                         };

//                         for frag_idx in 0..frag.total_n_fragments {
//                             let ack = Ack { fragment_index: frag_idx };
//                             let ack_pkt = Packet {
//                                 routing_header: base_reply_header.clone(),
//                                 session_id,
//                                 pack_type: PacketType::Ack(ack),
//                             };

//                             if rev_hops.len() >= 2 {
//                                 let next = rev_hops[1];
//                                 if let Some(chan) = self.packet_send.get(&next) {
//                                     let _ = chan.send(ack_pkt.clone());
//                                 } else {
//                                     eprintln!(
//                                         "ERROR: no channel to send ACK({}) to {} (rev_hops={:?})",
//                                         frag_idx, next, rev_hops
//                                     );
//                                 }
//                             } else {
//                                 eprintln!("ERROR: cannot send ACK, reversed path too short: {:?}", rev_hops);
//                             }
//                         }

//                         // Finally, remove the buffer and last_header entry:
//                         self.buffers().remove(&(session_id, src_id));
//                         self.last_header().remove(&(session_id, src_id));
//                     }

//                     // Done handling “final‐destination” case:
//                     return;
//                 }

//                 // ─────────────────────────────────────────────────────────────────────
//                 // 1.d) Otherwise, we are *not* the final hop. Forward the fragment:

//                 // Determine the next hop:
//                 if header.hop_index < header.hops.len() {
//                     let next_hop = header.hops[header.hop_index];

//                     // If next_hop is not one of our neighbors → NACK(ErrorInRouting(next_hop)):
//                     if !self.packet_send.contains_key(&next_hop) {
//                         let err = Nack {
//                             fragment_index: frag.fragment_index,
//                             nack_type: NackType::ErrorInRouting(next_hop),
//                         };
//                         let mut rev_hops = header.hops.clone();
//                         rev_hops.reverse();
//                         let reply_header = SourceRoutingHeader {
//                             hops: rev_hops.clone(),
//                             hop_index: 0,
//                         };
//                         let nack_pkt = Packet {
//                             routing_header: reply_header.clone(),
//                             session_id,
//                             pack_type: PacketType::Nack(err),
//                         };

//                         if rev_hops.len() >= 2 {
//                             let back_hop = rev_hops[1];
//                             if let Some(chan) = self.packet_send.get(&back_hop) {
//                                 let _ = chan.send(nack_pkt);
//                             } else {
//                                 eprintln!(
//                                     "ERROR: no channel to send NACK to {} (rev_hops={:?})",
//                                     back_hop, rev_hops
//                                 );
//                             }
//                         } else {
//                             eprintln!("ERROR: cannot send NACK, reversed path too short: {:?}", rev_hops);
//                         }
//                         return;
//                     }

//                     // Otherwise, forward the fragment onward:
//                     let forward_pkt = Packet {
//                         routing_header: header.clone(),
//                         session_id,
//                         pack_type: PacketType::MsgFragment(frag.clone()),
//                     };
//                     if let Some(chan) = self.packet_send.get(&next_hop) {
//                         let _ = chan.send(forward_pkt);
//                     } else {
//                         eprintln!(
//                             "ERROR: no channel to forward fragment {} to {}",
//                             frag.fragment_index, next_hop
//                         );
//                     }
//                 }
//             }
//             PacketType::Ack(_) => {
//                 // Servers typically do not need to re‐forward ACKs. 
//                 // clear retransmission buffers here if you kept any.
                
//             }
//             PacketType::Nack(nack) => {
//                 //if this is a DestinationIsDrone Nack... drop it 
//                 if let NackType::DestinationIsDrone = nack.nack_type{
//                     return;
//                 }
                
//                 let mut header= pkt.routing_header.clone();

//                 //verify correct hop or not
//                 if header.hops[header.hop_index] != self.id{
//                     //send ErrorInRouting and return
//                     return;
//                 }

//                 //advance hop_index
//                 header.hop_index +=1;

//                 //if server is at the end ...return
//                 if header.hop_index == header.hops.len(){
//                     return;
//                 }
                
//                 //otherwise forward nack to next_hop
//                 let next_hop = header.hops[header.hop_index];
//                 let nack_pkt = Packet{
//                     routing_header: header,
//                     session_id: pkt.session_id,
//                     pack_type: PacketType::Nack(nack.clone())
//                 };
                
//                 if let Some(chan)= self.packet_send.get(&next_hop){
//                     let _ =chan.send(nack_pkt);
//                 }
//                 else {
//                     eprintln!(
//                         "ERROR: no channel to forward NACK(fragment_index={}) to {}",
//                         nack.fragment_index, next_hop
//                     );
//                 }
//             }
//             PacketType::FloodRequest(fq) => {
//                 self.handle_flood_request(fq, &pkt.routing_header);
//             }
//             PacketType::FloodResponse(fr) => {
//                 self.handle_flood_response(fr, &pkt.routing_header);
//             }
//         }
//     }
// }


// fn main() {
//     let mut server = ChatServer::new();
//     server.on_request_arrived(1, 1, ChatRequest::Register(1).stringify());
//     server.on_request_arrived(
//         1,
//         1,
//         ChatRequest::SendMessage {
//             from: 1,
//             to: 2,
//             message: "Hello".to_string(),
//         }
//         .stringify(),
//     );
//     server.on_request_arrived(1, 1, "ServerType".to_string());
// }
