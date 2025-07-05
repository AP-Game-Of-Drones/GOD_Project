# 🧠 ChatClient & WebBrowser

The chat client and web browser are "different" implementation of client, 
used for low level logic of ChatApp and WebApp 
("the implementation of everything apart for the inherent Requests and Responses
is duplicated inside each of them")

---

## 📦 Architecture Overview
  - **ChatClient**
    ```rust
    #[derive(Clone)]
    pub struct ChatClient {
        id: NodeId,                                   // Unique identifier for the client
        controller_send: Sender<NodeEvent>, // Sender for communication with the controller
        controller_recv: Receiver<NodeCommand>, // Receiver for commands from the controller
        packet_recv: Receiver<Packet>,      // Receiver for incoming packets
        packet_send: HashMap<NodeId, Sender<Packet>>, // Map of packet senders for neighbors
        flood_ids: HashSet<(u64, NodeId)>,  // Set to track flood IDs for deduplication
        client_topology: super::super::topology::Topology, // topology built by flooding
        holder_sent: HashMap<(u64, NodeId), Vec<Packet>>, //fragment holder of sent messages, use session_id,src_id tuple as key
        holder_frag_index: HashMap<(u64, NodeId), Vec<u64>>, //fragment indices holder for received packets, use session_id,src_id tuple as key
        holder_rec: HashMap<(u64, NodeId), Vec<u8>>, //data holder of received messages, use session_id,src_id tuple as key
        registered_to: Vec<NodeId>,
        chat_servers: Vec<NodeId>,
        chat_contacts: Vec<(NodeId, NodeId)>,
        sent: HashMap<(u64, u8), Message>,
        gui_command_receiver: Receiver<ChatCommand>,
        gui_event_sender: Sender<ChatEvent>,
    }  
    ```
  - **WeBBrowser**
    ```rust
    pub struct WebBrowser {
        id: NodeId,                                   // Unique identifier for the client
        controller_send: Sender<NodeEvent>, // Sender for communication with the controller
        controller_recv: Receiver<NodeCommand>, // Receiver for commands from the controller
        packet_recv: Receiver<Packet>,      // Receiver for incoming packets
        packet_send: HashMap<NodeId, Sender<Packet>>, // Map of packet senders for neighbors
        flood_ids: HashSet<(u64, NodeId)>,  // Set to track flood IDs for deduplication
        client_topology: super::super::topology::Topology, // topology built by flooding
        holder_sent: HashMap<(u64, NodeId), Vec<Packet>>, //fragment holder of sent messages, use session_id,src_id tuple as key
        holder_frag_index: HashMap<(u64, NodeId), Vec<u64>>, //fragment indices holder for received packets, use session_id,src_id tuple as key
        holder_rec: HashMap<(u64, NodeId), Vec<u8>>, //data holder of received messages, use session_id,src_id tuple as key
        pre_processed: Option<((u64, NodeId), Message)>,
        sent: HashMap<(u64, u8), Message>,
        text_servers: Vec<NodeId>,
        media_servers: Vec<NodeId>,
        media: HashMap<(u64, u8), Message>,
        text: HashMap<(u64, u8), Vec<String>>,
        gui_command_receiver: Receiver<WebCommand>,
        gui_event_sender: Sender<WebEvent>,
    }  
    ```

---
