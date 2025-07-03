use super::super::controller::*;

pub fn spawn(simulation_controller: &mut SimulationController, val: usize, id: u8) {
    let (drone_command_sender, drone_command_receiver) = unbounded::<DroneCommand>();
    simulation_controller
        .sender_drone_command
        .insert(id, drone_command_sender);

    let (drone_packet_sender, drone_packet_receiver) = unbounded::<Packet>();
    simulation_controller
        .sender_node_packet
        .insert(id, drone_packet_sender);

    let sender_clone = simulation_controller.sender_drone_event.clone();
    let packet_senders: HashMap<NodeId, Sender<Packet>> = HashMap::new();

    let pdr = 0.0;
    let mut handles = Vec::new();
    handles.push(thread::spawn(move || {
        //TODO non credo serva  joinare thread, ma non sono sicuro
        match val {
            0 => {
                info!("BagelBomber Id[{}]", id);
                let mut drone = bagel_bomber::BagelBomber::new(
                    id,
                    sender_clone,
                    drone_command_receiver,
                    drone_packet_receiver,
                    packet_senders,
                    pdr,
                );
                drone.run();
            }
            1 => {
                info!("BetteCallDrone Id[{}]", id);
                let mut drone = drone_bettercalldrone::BetterCallDrone::new(
                    id,
                    sender_clone,
                    drone_command_receiver,
                    drone_packet_receiver,
                    packet_senders,
                    pdr,
                );
                drone.run();
            }
            2 => {
                info!("RustRoveri Id[{}]", id);
                let mut drone = rust_roveri::drone::RustRoveri::new(
                    id,
                    sender_clone,
                    drone_command_receiver,
                    drone_packet_receiver,
                    packet_senders,
                    pdr,
                );
                drone.run();
            }
            3 => {
                info!("GetDroned Id[{}]", id);
                let mut drone = getdroned::GetDroned::new(
                    id,
                    sender_clone,
                    drone_command_receiver,
                    drone_packet_receiver,
                    packet_senders,
                    pdr,
                );
                drone.run();
            }
            4 => {
                info!("C++Enjoyers Id[{}]", id);
                let mut drone = ap2024_unitn_cppenjoyers_drone::CppEnjoyersDrone::new(
                    id,
                    sender_clone,
                    drone_command_receiver,
                    drone_packet_receiver,
                    packet_senders,
                    pdr,
                );
                drone.run();
            }
            5 => {
                info!("D.R.O.N.E Id[{}]", id);
                let mut drone = d_r_o_n_e_drone::MyDrone::new(
                    id,
                    sender_clone,
                    drone_command_receiver,
                    drone_packet_receiver,
                    packet_senders,
                    pdr,
                );
                drone.run();
            }
            6 => {
                info!("NNP Id[{}]", id);
                let mut drone = null_pointer_drone::MyDrone::new(
                    id,
                    sender_clone,
                    drone_command_receiver,
                    drone_packet_receiver,
                    packet_senders,
                    pdr,
                );
                drone.run();
            }
            7 => {
                info!("Rustafarian Id[{}]", id);
                let mut drone = rustafarian_drone::RustafarianDrone::new(
                    id,
                    sender_clone,
                    drone_command_receiver,
                    drone_packet_receiver,
                    packet_senders,
                    pdr,
                );
                drone.run();
            }
            8 => {
                info!("DrOnes[{}]", id);
                let mut drone = dr_ones::Drone::new(
                    id,
                    sender_clone,
                    drone_command_receiver,
                    drone_packet_receiver,
                    packet_senders,
                    pdr,
                );
                drone.run();
            }
            9 => {
                info!("Rusteze Id[{}]", id);
                let mut drone = rusteze_drone::RustezeDrone::new(
                    id,
                    sender_clone,
                    drone_command_receiver,
                    drone_packet_receiver,
                    packet_senders,
                    pdr,
                );
                drone.run();
            }
            _ => {
                info!("Error modulo");
            }
        }
    }));
}

pub fn crash(simulation_controller: &mut SimulationController, id: u8, neighbours: Vec<u8>) {
    match simulation_controller
        .sender_drone_command
        .get(&id)
        .unwrap()
        .send(DroneCommand::Crash)
    {
        Ok(_) => {
            simulation_controller.sender_drone_command.remove(&id);
            info!("Sent crash to {}", id);
            for neighbour in neighbours {
                remove_sender(simulation_controller, neighbour, id);
            }
        }
        Err(_) => {
            warn!("crash to {} error", id);
        }
    }
}

pub fn remove_sender(simulation_controller: &mut SimulationController, id: u8, removed_id: u8) {
    if let Some(drone) = simulation_controller.sender_drone_command.get(&id) {
        match drone.send(DroneCommand::RemoveSender(removed_id)) {
            Ok(_) => {
                info!("Sent remove_sender to drone {}", id);
            }
            Err(_) => {
                warn!("remove_sender to drone {} error", id);
            }
        }
    }
    if let Some(client_server) = simulation_controller.sender_client_server_command.get(&id) {
        match client_server.send(NodeCommand::RemoveSender(removed_id)) {
            Ok(_) => {
                info!("Sent remove_sender to client or server {}", id);
            }
            Err(_) => {
                warn!("remove_sender to client or server {} error", id);
            }
        }
    }
}

pub fn add_sender(simulation_controller: &mut SimulationController, id: u8, receiver_id: u8) {
    let sender = simulation_controller
        .sender_node_packet
        .get(&receiver_id)
        .unwrap();

    if let Some(drone) = simulation_controller.sender_drone_command.get(&id) {
        match drone.send(DroneCommand::AddSender(receiver_id, sender.clone())) {
            Ok(_) => {
                info!("Sent add_sender to drone {}", id);
            }
            Err(_) => {
                warn!("add_sender to drone {} error", id);
            }
        }
    }
    if let Some(client_server) = simulation_controller.sender_client_server_command.get(&id) {
        match client_server.send(NodeCommand::AddSender(receiver_id, sender.clone())) {
            Ok(_) => {
                info!("Sent add_sender to client or server {}", id);
            }
            Err(_) => {
                warn!("add_sender to client or server {} error", id);
            }
        }
    }
}
pub fn set_pdr(simulation_controller: &mut SimulationController, id: u8, pdr: f32) {
    match simulation_controller
        .sender_drone_command
        .get(&id)
        .unwrap()
        .send(DroneCommand::SetPacketDropRate(pdr))
    {
        Ok(_) => {
            info!("Sent set_pdr to {}", id);
        }
        Err(_) => {
            warn!("set_pdr to {} error", id);
        }
    }
}
