#![allow(dead_code)]
use crate::{
    frontend::{
        self,
        chat_gui::{self, GuiChannels},
    },
    utils::{client::chat_client::ChatClient, backup_server},
};
use bevy::scene::ron::error;
use crossbeam_channel::*;
use rand::*;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs,
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
};
use toml::{self};
use wg_2024::{
    config::Config,
    controller::{DroneCommand, DroneEvent},
    drone::Drone,
    network::NodeId,
    packet::{NodeType, Packet},
};

use super::client::*;
use super::backup_server::*;
use super::simulation_controller::*;

fn parse_config(file: &str) -> Config {
    let file_str = fs::read_to_string(file).unwrap();
    toml::from_str(&file_str).unwrap()
}

fn astor(counters: &mut [i32]) -> usize {
    let mut val = 0;
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

fn foreach(counters: &mut [i32]) -> usize {
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

fn build(
    id: NodeId,
    controller_drone_recv: crossbeam_channel::Receiver<DroneCommand>,
    node_event_send: crossbeam_channel::Sender<DroneEvent>,
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
                node_event_send,
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
                node_event_send,
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
                node_event_send,
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
                node_event_send,
                controller_drone_recv,
                packet_recv,
                packet_send,
                pdr,
            );
            drone.run();
        }
        4 => {
            // println!("C++Enjoyers Id[{}]",id);
            // let mut drone = ap2024_unitn_cppenjoyers_drone::CppEnjoyersDrone::new(id, node_event_send, controller_drone_recv, packet_recv, packet_send, pdr);
            // drone.run();
        }
        5 => {
            println!("D.R.O.N.E Id[{}]", id);
            let mut drone = d_r_o_n_e_drone::MyDrone::new(
                id,
                node_event_send,
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
                node_event_send,
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
                node_event_send,
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
                node_event_send,
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
                node_event_send,
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

pub fn initialize(
    path_to_file: &str,
) -> Result<(Vec<JoinHandle<()>>, HashMap<u8, GuiChannels>), Box<dyn std::error::Error + Send + Sync>>
{
    // let mut drones_mutex = Vec::new();
    // let mut clients_mutex = Vec::new();
    // let mut servers_mutex = Vec::new();

    let config = parse_config(path_to_file);
    // println!("{:?}", config.drone.clone());
    // println!("{:?}", config.client.clone());

    let mut dd = HashMap::new();
    let mut controller_drones = HashMap::new();
    let (node_event_send, node_event_recv) = unbounded();
    let mut cs_controller = HashMap::new();
    let (cs_send, cs_recv) = unbounded::<NodeEvent>();

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

    let mut counters = [0; 10];
    // let mut handles_c = Vec::new();
    let len = config.drone.len();
    println!("{}", len);
    for drone in config.drone.into_iter() {
        // controller
        let (controller_drone_send, controller_drone_recv) = unbounded();
        controller_drones.insert(drone.id, controller_drone_send);
        let node_event_send = node_event_send.clone();
        // packet
        let packet_recv = packet_channels[&drone.id].1.clone();
        let packet_send = drone
            .connected_node_ids
            .clone()
            .into_iter()
            .map(|id| (id, packet_channels[&id].0.clone()))
            .collect();
        dd.insert(drone.id, drone.connected_node_ids.clone());
        let val;
        if len < 10 {
            val = astor(&mut counters);
        } else {
            val = foreach(&mut counters);
        }
        handles.push(thread::spawn(move || {
            build(
                drone.id,
                controller_drone_recv,
                node_event_send,
                packet_recv,
                packet_send,
                drone.pdr,
                val,
            );
        }));
    }
    let mut gui = HashMap::new();
    // let mut clients = Vec::new();
    for drone in config.client.into_iter() {
        // controller
        let (controller_drone_send, controller_drone_recv) = unbounded::<NodeCommand>();
        let (gui_event_sender, gui_event_receiver) = unbounded::<frontend::ChatEvent>();
        let (gui_command_sender, gui_command_receiver) = unbounded::<frontend::ChatCommand>();
        let gui_channels = GuiChannels::new(gui_event_receiver, gui_command_sender);
        gui.insert(drone.id, gui_channels);
        cs_controller.insert(drone.id, controller_drone_send);
        let cs_send = cs_send.clone();
        // packet
        let packet_recv = packet_channels[&drone.id].1.clone();
        let packet_send = drone
            .connected_drone_ids
            .into_iter()
            .map(|id| (id, packet_channels[&id].0.clone()))
            .collect();

        handles.push(thread::spawn(move || {
            // let arc = Arc::new(Mutex::new(ChatClient::new(
            //     drone.id,
            //     cs_send,
            //     controller_drone_recv,
            //     packet_recv,
            //     packet_send,
            //     gui_command_receiver,
            //     gui_event_sender
            // )));
            // thread::spawn(move || {
            //     Arc::clone(&arc).lock().unwrap().handle_channels();
            // });
            let mut client = ChatClient::new(
                drone.id,
                cs_send,
                controller_drone_recv,
                packet_recv,
                packet_send,
                gui_command_receiver,
                gui_event_sender,
            );
            client.handle_channels();
        }));
    }

    // let mut servers = Vec::new();

    for drone in config.server.into_iter() {
        // controller
        let (controller_drone_send, controller_drone_recv) = unbounded();
        cs_controller.insert(drone.id, controller_drone_send);
        let cs_send = cs_send.clone();
        // packet
        let packet_recv = packet_channels[&drone.id].1.clone();
        let packet_send = drone
            .connected_drone_ids
            .into_iter()
            .map(|id| (id, packet_channels[&id].0.clone()))
            .collect();

        handles.push(thread::spawn(move || {
            let mut server = backup_server::Server::new(
                drone.id,
                3,
                cs_send,
                controller_drone_recv,
                packet_recv,
                packet_send,
            );
            server.handle_channels();
        }));
    }

    handles.push(thread::spawn(move || {
        let mut controller = SimulationController {
            droness: dd,
            drones: controller_drones,
            node_event_recv,
            cli_ser_send: cs_controller,
            cli_ser_recv: cs_recv,
        };
        loop {}
    }));
    Ok((handles, gui))
}

fn check_neighbors_id(current: NodeId, neighbors: &Vec<NodeId>) -> bool {
    neighbors.into_iter().all(|f| *f != current)
        && (neighbors.iter().copied().collect::<HashSet<_>>().len() == neighbors.len())
}

fn check_pdr(pdr: f32) -> bool {
    pdr >= 0.0 && pdr <= 1.00
}

fn check_initializer(path_to_file: &str) -> bool {
    let config_data = std::fs::read_to_string(path_to_file).expect("Unable to read config file");
    // having our structs implement the Deserialize trait allows us to use the toml::from_str function to deserialize the config file into each of them
    let config: Config = toml::from_str(&config_data).expect("Unable to parse TOML");
    let mut current;
    let mut last = 0;
    let mut res = true;
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
            if check_neighbors_id(current, &client.connected_drone_ids) {
                if current != last {
                    last = client.id;
                } else {
                    res = false;
                }
            } else {
                res = false;
            }
        }
        if res {
            for server in config.server {
                current = server.id;
                if check_neighbors_id(current, &server.connected_drone_ids) {
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
    }
    res
}

#[cfg(test)]
mod test {
    use wg_2024::config;

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

    #[test]
    fn test_init_diff() {
        while let Some(h) = initialize("./configs/config.toml").unwrap().0.pop() {
            assert_eq!(1, 2);
            h.join();
        }
    }

    // #[test]
    fn check_build() {
        let config = parse_config("./configs/config.toml");
        let mut dd = HashMap::new();
        let mut controller_drones = HashMap::new();
        let (node_event_send, node_event_recv) = unbounded();
        // let mut cs_controller = HashMap::new();
        let (cs_send, cs_recv) = unbounded::<NodeEvent>();

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

        let mut counters = [0; 10];
        // let mut handles_c = Vec::new();
        let len = config.drone.len();
        for drone in config.drone.into_iter() {
            // controller
            let (controller_drone_send, controller_drone_recv) = unbounded();
            controller_drones.insert(drone.id, controller_drone_send);
            let node_event_send = node_event_send.clone();
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
                //     node_event_send,
                //     controller_drone_recv,
                //     packet_recv,
                //     packet_send,
                //     drone.pdr,
                // );

                build(
                    drone.id,
                    controller_drone_recv,
                    node_event_send,
                    packet_recv,
                    packet_send,
                    drone.pdr,
                    0,
                );

                // println!("{}  {:?}", drone.id, drone.packet_send.clone());

                // wg_2024::drone::Drone::run(&mut drone);
            }));
        }
        while let Some(d) = handles.pop() {
            d.join();
        }
        assert_eq!(1, 2);
    }
}
