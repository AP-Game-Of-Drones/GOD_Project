#![allow(dead_code)]
use crate::{
    frontend::{ChatCommand, ChatEvent, WebCommand, WebEvent},
    utils::{
        client::{chat_client::ChatClient, web_browser::WebBrowser},
        controller::{NodeCommand, NodeEvent},
    },
};
use crossbeam_channel::*;
use rand::*;
use std::{
    collections::{HashMap, HashSet, VecDeque}, fs, io::Write, path::PathBuf, thread::{self, JoinHandle}
};
use toml::{self};
use wg_2024::{
    config::{Client, Config, Server},
    controller::{DroneCommand, DroneEvent},
    drone::Drone,
    network::NodeId,
    packet::Packet,
};

fn parse_config(file: &str) -> Config {
    println!("{file}");
    let file_str = fs::read_to_string(file).unwrap();
    toml::from_str(&file_str).unwrap()
}

const CHATAPP: u8 = 0;
const WEBAPP: u8 = 1;

fn helper1(counters: &mut [i32]) -> usize {
    let mut val ;
    loop {
        let mut rng = rand::thread_rng();
        val = rng.r#gen::<usize>() % 10;
        if *counters.iter().min().unwrap() == counters[val] {
            counters[val] += 1;
            if *counters.iter().max().unwrap() - *counters.iter().min().unwrap() == 1 {
                break;
            } else {
                counters[val] -= 1;
            }
        }
    }
    val
}

fn helper2(counters: &mut [i32]) -> usize {
    let mut val;
    loop {
        let mut rng = rand::thread_rng();
        val = rng.r#gen::<usize>() % 10;
        if counters[val] != 1 {
            // println!("Value:[{}], Counters:[{:?}]",val,counters);
            counters[val] = 1;
            break;
        }
    }
    val
}

fn helper3(config: &Config) -> u8 {
    let s_len = config.server.len();
    let c_len = config.client.len();
    let mut rng = rand::thread_rng();
    let val = rng.r#gen::<u8>();
    let mut res = 0;
    if c_len == 1  && s_len>1{
        res = WEBAPP;
    } else if c_len == 2 {
        if s_len == 1 {
            res = CHATAPP;
        } else if s_len > 1 {
            println!("Choose:\n\t1 for ChatApp\n\t2 for WebApp ");
            std::io::stdout().flush().unwrap();
            let mut str =  String::new();
            std::io::stdin().read_line(&mut str).ok();
            match str.trim().parse::<u8>(){
                Ok(n)=>{
                    res = n;
                }, 
                Err(_)=>{
                    res = val % 2;
                }
            }
        }
    } else if c_len > 3 {
        if s_len <= 1 {
            res = CHATAPP
        } else if s_len >= 2 {
            println!("Choose:\n\t1 for ChatApp\n\t2 for WebApp ");
            std::io::stdout().flush().unwrap();
            let mut str =  String::new();
            std::io::stdin().read_line(&mut str).ok();
            match str.trim().parse::<u8>(){
                Ok(n)=>{
                    res = n;
                }, 
                Err(_)=>{
                    res = val % 2;
                }
            }
        }
    }
    res
}

pub fn choose_config_cli()->(PathBuf,bool){
    let config_path = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
        .parent()
        .unwrap()
        .to_path_buf()
        .parent()
        .unwrap()
        .to_path_buf()
        .join("configs/");
    println!("Chose from the following configuration files by entering the corresponding number");    
    let reader = std::fs::read_dir(config_path).expect("No config dir found");
    let configs = reader.into_iter().enumerate().map(|(i,c)| (i,c.ok().unwrap().path())).collect::<HashMap<usize,PathBuf>>();
    for (i, entry) in &configs {
        println!("{}\t{}", i, entry.to_string_lossy());
    }

    std::io::stdout().flush().unwrap();

    loop {
        print!("Choose a config by number: ");
        std::io::stdout().flush().unwrap();

        let mut num = String::new();
        std::io::stdin().read_line(&mut num).unwrap();

        // Try to parse input to a number
        match num.trim().parse::<usize>() {
            Ok(n) => {
                if let Some(c) = configs.get(&n) {
                    println!("Chosen: {}", c.to_string_lossy());
                    if check_initializer(c.to_str().unwrap()) {
                        println!("Choose 1 to use just one impl for the drones, choose 2 for multiple impl");
                        std::io::stdout().flush().unwrap();
                        let mut str = String::new();
                        std::io::stdin().read_line(&mut str).unwrap();
                        match str.trim().parse::<u8>(){
                            Ok(m) => {
                                if m == 1 {
                                    return (c.clone(),true);
                                } else if m == 2 {
                                    return (c.clone(),false);
                                } else {
                                    println!("Chose between 1 and 2");
                                }
                            },
                            Err(_)=>{
                                println!("Enter a valid number");
                            }
                        }
                    } else {
                        println!("Config file doesn't respect protocol")
                    }
                } else {
                    println!("Invalid number: No config at index {}", n);
                }
            }
            Err(_) => {
                println!("Please enter a valid number.");
            }
        }
    }
        

}

fn build(
    id: NodeId,
    controller_drone_recv: crossbeam_channel::Receiver<DroneCommand>,
    drone_event_send: crossbeam_channel::Sender<DroneEvent>,
    packet_recv: crossbeam_channel::Receiver<Packet>,
    packet_send: HashMap<NodeId, crossbeam_channel::Sender<Packet>>,
    pdr: f32,
    val: usize,
) {
    match val {
        0 => {
            println!("BagelBomber Id[{}]", id);
            let mut drone = bagel_bomber::BagelBomber::new(
                id,
                drone_event_send,
                controller_drone_recv,
                packet_recv,
                packet_send,
                pdr,
            );
            drone.run();
        }
        1 => {
            println!("BetteCallDrone Id[{}]", id);
            let mut drone = drone_bettercalldrone::BetterCallDrone::new(
                id,
                drone_event_send,
                controller_drone_recv,
                packet_recv,
                packet_send,
                pdr,
            );
            drone.run();
        }
        2 => {
            println!("RustRoveri Id[{}]", id);
            let mut drone = rust_roveri::drone::RustRoveri::new(
                id,
                drone_event_send,
                controller_drone_recv,
                packet_recv,
                packet_send,
                pdr,
            );
            drone.run();
        }
        3 => {
            println!("GetDroned Id[{}]", id);
            let mut drone = getdroned::GetDroned::new(
                id,
                drone_event_send,
                controller_drone_recv,
                packet_recv,
                packet_send,
                pdr,
            );
            drone.run();
        }
        4 => {
            println!("C++Enjoyers Id[{}]", id);
            let mut drone = ap2024_unitn_cppenjoyers_drone::CppEnjoyersDrone::new(
                id,
                drone_event_send,
                controller_drone_recv,
                packet_recv,
                packet_send,
                pdr,
            );
            drone.run();
        }
        5 => {
            println!("D.R.O.N.E Id[{}]", id);
            let mut drone = d_r_o_n_e_drone::MyDrone::new(
                id,
                drone_event_send,
                controller_drone_recv,
                packet_recv,
                packet_send,
                pdr,
            );
            drone.run();
        }
        6 => {
            println!("NNP Id[{}]", id);
            let mut drone = null_pointer_drone::MyDrone::new(
                id,
                drone_event_send,
                controller_drone_recv,
                packet_recv,
                packet_send,
                pdr,
            );
            drone.run();
        }
        7 => {
            println!("Rustafarian Id[{}]", id);
            let mut drone = rustafarian_drone::RustafarianDrone::new(
                id,
                drone_event_send,
                controller_drone_recv,
                packet_recv,
                packet_send,
                pdr,
            );
            drone.run();
        }
        8 => {
            println!("DrOnes[{}]", id);
            let mut drone = dr_ones::Drone::new(
                id,
                drone_event_send,
                controller_drone_recv,
                packet_recv,
                packet_send,
                pdr,
            );
            drone.run();
        }
        9 => {
            println!("Rusteze Id[{}]", id);
            let mut drone = rusteze_drone::RustezeDrone::new(
                id,
                drone_event_send,
                controller_drone_recv,
                packet_recv,
                packet_send,
                pdr,
            );
            drone.run();
        }
        _ => {
            println!("Error modulo");
        }
    }
}

fn build_and_run_client(
    id: NodeId,
    controller_send: Sender<NodeEvent>,
    controller_recv: Receiver<NodeCommand>,
    packet_recv: Receiver<Packet>,
    packet_send: HashMap<NodeId, Sender<Packet>>,
    app_value: u8,
) -> (
    Option<Sender<ChatCommand>>,
    Option<Receiver<ChatEvent>>,
    Option<Sender<WebCommand>>,
    Option<Receiver<WebEvent>>,
) {
    let (chat_events_sender, chat_events_receiver) = unbounded::<ChatEvent>();
    let (chat_commands_sender, chat_commands_receiver) = unbounded::<ChatCommand>();
    let (web_events_sender, web_events_receiver) = unbounded::<WebEvent>();
    let (web_commands_sender, web_commands_receiver) = unbounded::<WebCommand>();
    let (mut chat_sender, mut chat_receiver, mut web_sender, mut web_receiver) =
        (None, None, None, None);
    if app_value == CHATAPP {
        thread::spawn(move || {
            let mut client = ChatClient::new(
                id,
                controller_send,
                controller_recv,
                packet_recv,
                packet_send,
                chat_commands_receiver,
                chat_events_sender,
            );
            client.handle_channels();
        });
        (chat_sender, chat_receiver) = (Some(chat_commands_sender), Some(chat_events_receiver));
    } else if app_value == WEBAPP {
        thread::spawn(move || {
            let mut client = WebBrowser::new(
                id,
                controller_send,
                controller_recv,
                packet_recv,
                packet_send,
                web_commands_receiver,
                web_events_sender,
            );
            client.handle_channels();
        });
        (web_sender, web_receiver) = (Some(web_commands_sender), Some(web_events_receiver));
    }
    (chat_sender, chat_receiver, web_sender, web_receiver)
}

fn build_and_run_server(
    id: NodeId,
    controller_send: Sender<NodeEvent>,
    controller_recv: Receiver<NodeCommand>,
    packet_recv: Receiver<Packet>,
    packet_send: HashMap<NodeId, Sender<Packet>>,
    app_value: u8,
    last_type: u8,
) -> u8 {
    let mut serv_type = 0;
    if app_value == CHATAPP {
        serv_type = super::backup_server::CHATSERVER;
        thread::spawn(move || {
            let mut server = super::backup_server::Server::new(
                id,
                serv_type,
                controller_send,
                controller_recv,
                packet_recv,
                packet_send,
            );
            server.handle_channels();
        });
    } else if app_value == WEBAPP {
        if last_type == super::backup_server::MEDIASERVER {
            serv_type = super::backup_server::TEXTSERVER;
        } else if last_type == super::backup_server::TEXTSERVER {
            serv_type = super::backup_server::MEDIASERVER;
        } else {
            serv_type = 1;
        }
        thread::spawn(move || {
            let mut server = super::backup_server::Server::new(
                id,
                serv_type,
                controller_send,
                controller_recv,
                packet_recv,
                packet_send,
            );
            server.handle_channels();
        });
    }
    return serv_type;
}

pub fn initialize(
    path_to_file: &str,
    the_one: bool,
) -> Result<
    (
        Vec<JoinHandle<()>>,
        HashMap<u8, super::super::frontend::chat_gui::GuiChannels>,
        HashMap<u8, super::super::frontend::web_gui::GuiChannels>,
        super::controller::SimulationController,
        Config,
    ),
    Box<dyn std::error::Error + Send + Sync>,
> {
    let config = parse_config(path_to_file);
    let app_magic_value = helper3(&config);

    let mut dd = HashMap::new();
    let mut controller_drones = HashMap::new();
    let (drone_event_send, drone_event_recv) = unbounded();
    let mut cs_controller = HashMap::new();
    let (cs_send, cs_recv) = unbounded::<NodeEvent>();

    let mut packet_channels = HashMap::new();
    //create channels for every node
    for drone in config.drone.iter() {
        packet_channels.insert(drone.id, unbounded()); 
    }
    for client in config.client.iter() {
        packet_channels.insert(client.id, unbounded());
    }
    for server in config.server.iter() {
        packet_channels.insert(server.id, unbounded());
    }

    let senders = packet_channels
        .clone()
        .into_iter()
        .map(|(id, (s, _r))| (id, s.clone()))
        .collect::<HashMap<u8, Sender<Packet>>>();

    let mut handles = Vec::new();

    let mut counters = [0; 10];
    let len = config.drone.len();
    let mut str = String::new();
    let mut val = 10;
    if the_one {
        println!("Choose for one implementation only:\n
                 \t-0: BagelBomber
                 \t-1: BetterCallDrone
                 \t-2: RustRoveri
                 \t-3: GetDroned
                 \t-4: C++Enjoyers
                 \t-5: D.R.O.N.E
                 \t-6: NNP
                 \t-7: Rustafarian
                 \t-8: DrOnes
                 \t-9: Rusteze\n
        ");
        loop {
            std::io::stdout().flush().unwrap();
            std::io::stdin().read_line(&mut str).unwrap();
            match str.trim().parse::<usize>(){
                Ok(n)=>{
                    if n<=9 {
                        val=n;
                        break;
                    } else{
                        println!("Choose a number between 0 and 9");
                    }
                },
                Err(_)=>{
                    println!("Enter a valid input");
                }
            }
        }
    }
    for drone in config.clone().drone.into_iter() {
        // controller
        let (controller_drone_send, controller_drone_recv) = unbounded();
        controller_drones.insert(drone.id, controller_drone_send.clone());
        let drone_event_send = drone_event_send.clone();
        // packet
        let packet_recv = packet_channels[&drone.id].1.clone();
        let packet_send = drone
            .connected_node_ids
            .clone()
            .into_iter()
            .map(|id| (id, packet_channels[&id].0.clone()))
            .collect();
        dd.insert(drone.id, drone.connected_node_ids.clone());
        if !the_one {
            if len <= 10 {
                val = helper2(&mut counters);
            } else {
                val = helper1(&mut counters);
            }
        }
        handles.push(thread::spawn(move || {
            build(
                drone.id,
                controller_drone_recv,
                drone_event_send,
                packet_recv,
                packet_send,
                drone.pdr,
                val,
            );
        }));
    }
    let mut gui_web = HashMap::new();
    let mut gui_chat = HashMap::new();

    for drone in config.clone().client.into_iter() {
        // controller
        let (controller_node_send, controller_node_recv) = unbounded::<NodeCommand>();

        cs_controller.insert(drone.id, controller_node_send.clone());
        let cs_send = cs_send.clone();
        // packet
        let packet_recv = packet_channels[&drone.id].1.clone();
        let packet_send = drone
            .connected_drone_ids
            .into_iter()
            .map(|id| (id, packet_channels[&id].0.clone()))
            .collect();

        let (cs, ce, ws, we) = build_and_run_client(
            drone.id,
            cs_send,
            controller_node_recv,
            packet_recv,
            packet_send,
            app_magic_value,
        );
        if app_magic_value == CHATAPP {
            let chat_channels =
                super::super::frontend::chat_gui::GuiChannels::new(ce.unwrap(), cs.unwrap());
            gui_chat.insert(drone.id, chat_channels);
        }
        if app_magic_value == WEBAPP {
            let chat_channels =
                super::super::frontend::web_gui::GuiChannels::new(we.unwrap(), ws.unwrap());
            gui_web.insert(drone.id, chat_channels);
        }
    }

    let mut last = 0;
    for drone in config.server.clone().into_iter() {
        // controller
        let (controller_drone_send, controller_drone_recv) = unbounded();
        cs_controller.insert(drone.id, controller_drone_send.clone());
        let cs_send = cs_send.clone();
        // packet
        let packet_recv = packet_channels[&drone.id].1.clone();
        let packet_send = drone
            .connected_drone_ids
            .into_iter()
            .map(|id| (id, packet_channels[&id].0.clone()))
            .collect();
        let current = build_and_run_server(
            drone.id,
            cs_send,
            controller_drone_recv,
            packet_recv,
            packet_send,
            app_magic_value,
            last,
        );
        last = current;
    }

    let controller = super::controller::SimulationController::new(
        controller_drones,
        cs_controller,
        drone_event_recv,
        cs_recv,
        drone_event_send,
        senders,
    );
    Ok((handles, gui_chat, gui_web, controller, config.clone()))
}

fn check_neighbors_id(current: NodeId, neighbors: &Vec<NodeId>) -> bool {
    neighbors.into_iter().all(|f| *f != current)
        && (neighbors.iter().copied().collect::<HashSet<_>>().len() == neighbors.len())
}

fn check_pdr(pdr: f32) -> bool {
    pdr >= 0.0 && pdr <= 1.00
}

fn check_client_server_connection(clients: &Vec<Client>, servers: &Vec<Server>)->bool {
    let mut res = true;
    for client in clients {
        for server in servers {
            if server.connected_drone_ids.contains(&client.id) {
                res = false;
            } 
            if client.connected_drone_ids.contains(&server.id) {
                res = false;
            }
        }
    }
    res
}

fn check_bidirectionality(config: &Config)->bool {
    // Helper: build a map of id -> neighbors
    let mut all_entities: HashMap<u8, &Vec<u8>> = HashMap::new();

    for client in &config.client {
        all_entities.insert(client.id, &client.connected_drone_ids);
    }
    for server in &config.server {
        all_entities.insert(server.id, &server.connected_drone_ids);
    }
    for drone in &config.drone {
        all_entities.insert(drone.id, &drone.connected_node_ids);
    }
    
    // For each entity, check that each neighbor has this entity as a neighbor
    for (id, neighbors) in &all_entities {
        for neighbor_id in *neighbors {
            if let Some(neighbor_neighbors) = all_entities.get(neighbor_id) {
                if !neighbor_neighbors.contains(id) {
                    println!("❌ ID {} has neighbor {}, but not reciprocated.", id, neighbor_id);
                    return false;
                }
            } else {
                println!("⚠️ ID {} refers to non-existent neighbor {}", id, neighbor_id);
                return false;
            }
        }
    }

    println!("✅ All links are bidirectional.");
    true
}

fn is_connected(drones: &Vec<wg_2024::config::Drone>, c_ids: Vec<u8>, s_ids: Vec<u8>) -> bool {
    if drones.is_empty() {
        return true; // empty graph is trivially connected
    }
    
    // Build adjacency map: id -> neighbors
    let mut adjacency: HashMap<u8, Vec<u8>> = HashMap::new();
    for drone in drones {
        let mut vec = Vec::new();
        for id in drone.connected_node_ids.clone() {
            if !c_ids.contains(&id) && !s_ids.contains(&id) {
                vec.push(id);
            }
        }
        adjacency.insert(drone.id, vec.clone());
    }

    // BFS or DFS to traverse the graph
    let start_id = drones[0].id;
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back(start_id);

    while let Some(current) = queue.pop_front() {
        if visited.insert(current) {
            if let Some(neighbors) = adjacency.get(&current) {
                for neighbor in neighbors {
                    queue.push_back(*neighbor);
                }
            }
        }
    }

    // Check if we visited all drones
    visited.len() == drones.len()
}



fn check_initializer(path_to_file: &str) -> bool {
    let config_data = std::fs::read_to_string(path_to_file).expect("Unable to read config file");
    // having our structs implement the Deserialize trait allows us to use the toml::from_str function to deserialize the config file into each of them
    let config: Config = toml::from_str(&config_data).expect("Unable to parse TOML");


    let mut current;
    let mut last = 0;
    let mut res = true;
     
    if config.drone.len() < 2 {
        println!("Configs Restriction not met: Drones must be > 1 ");
        res = false;
    } else if (config.client.len()<2 && config.server.len()<2) 
            || config.client.len()==0 
            || config.server.len()==0  
    {
        println!("Configs Restriction not met: Servers must be > 1 and Client >2 for ChatApp; Servers must be > 2 and Client >1 for ChatApp;");
        res = false;
    } else {
        if check_bidirectionality(&config) {
            let c_ids = config.clone().client.into_iter().map(|c| c.id).collect::<Vec<u8>>();
            let s_ids = config.clone().server.into_iter().map(|s| s.id).collect::<Vec<u8>>();
            if is_connected(&config.drone,c_ids,s_ids){
                for drone in config.drone {
                    current = drone.id;
                    if check_neighbors_id(current, &drone.connected_node_ids) {
                        if check_pdr(drone.pdr) {
                            if current != last {
                                last = drone.id;
                            } else {
                                res = false;
                            }
                        } else {
                            res = false;
                        }
                    } else {
                        res = false;
                    }
                }
                if res {
                    for client in config.client {
                        current = client.id;
                        if client.connected_drone_ids.len()<1 || client.connected_drone_ids.len()>2 {
                            res = false;
                        } else if check_neighbors_id(current, &client.connected_drone_ids) {
                            if current != last {
                                last = client.id;
                            } else {
                                res = false;
                            }
                        } else {
                            res = false;
                        }
                    }
                
                }
                if res {
                    for server in config.server {
                        current = server.id;
                        if server.connected_drone_ids.len()<2 {
                            res = false;
                        } else if check_neighbors_id(current, &server.connected_drone_ids) {
                            if current != last {
                                last = server.id;
                            } else {
                                res = false;
                            }
                        } else {
                            res = false;
                        }
                    }
                }
            } else {
                res = false;
            }
        } else {
            res = false;
        }
    }       
    res
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_init() {
        assert_eq!(check_initializer("./configs/config.toml"), true);
    }

    #[test]
    fn test_pdr() {
        for pdr in 0..100 {
            assert_eq!(check_pdr((pdr / 100) as f32), true);
        }
    }

    #[test]
    fn test_neigbors() {
        let neighbors: Vec<u8> = [2, 3, 4, 5].to_vec();
        let neighbors_not: Vec<u8> = [2, 3, 4, 3].to_vec();
        assert_eq!(check_neighbors_id(1, &neighbors), true);
        assert_eq!(check_neighbors_id(4, &neighbors), false);
        assert_eq!(check_neighbors_id(1, &neighbors_not), false);
    }

    // #[test]
    fn check_build() {
        let config = parse_config("./configs/config.toml");
        let mut dd = HashMap::new();
        let mut controller_drones = HashMap::new();
        let (drone_event_send, _drone_event_recv) = unbounded();
        // let mut cs_controller = HashMap::new();
        let (_cs_send, _cs_recv) = unbounded::<NodeEvent>();

        let mut packet_channels = HashMap::new();
        for drone in config.drone.iter() {
            packet_channels.insert(drone.id, unbounded());
        }
        for client in config.client.iter() {
            packet_channels.insert(client.id, unbounded());
        }
        for server in config.server.iter() {
            packet_channels.insert(server.id, unbounded());
        }

        let mut handles = Vec::new();

        // let mut handles_c = Vec::new();
        let _len = config.drone.len();
        for drone in config.drone.into_iter() {
            // controller
            let (controller_drone_send, controller_drone_recv) = unbounded();
            controller_drones.insert(drone.id, controller_drone_send);
            let drone_event_send = drone_event_send.clone();
            // packet
            let packet_recv = packet_channels[&drone.id].1.clone();
            let packet_send = drone
                .connected_node_ids
                .clone()
                .into_iter()
                .map(|id| (id, packet_channels[&id].0.clone()))
                .collect();
            dd.insert(drone.id, drone.connected_node_ids.clone());
            handles.push(thread::spawn(move || {
                // let mut drone = null_pointer_drone::MyDrone::new(
                //     drone.id,
                //     drone_event_send,
                //     controller_drone_recv,
                //     packet_recv,
                //     packet_send,
                //     drone.pdr,
                // );

                build(
                    drone.id,
                    controller_drone_recv,
                    drone_event_send,
                    packet_recv,
                    packet_send,
                    drone.pdr,
                    0,
                );

                // println!("{}  {:?}", drone.id, drone.packet_send.clone());

                // wg_2024::drone::Drone::run(&mut drone);
            }));
        }
        assert_eq!(1, 2);
    }
}
