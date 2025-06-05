use crossbeam_channel::*;
use std::collections::{HashMap, HashSet};
use wg_2024::{controller::*, network::*, packet::*};

pub enum NodeCommand {
    SendPacket(Packet),
}

pub enum NodeEvent {
    SentPacket(Packet),
}

pub struct SimulationController {
    pub droness: HashMap<NodeId, Vec<NodeId>>,
    pub drones: HashMap<NodeId, Sender<DroneCommand>>,
    pub node_event_recv: Receiver<DroneEvent>,
    pub cli_ser_send: HashMap<NodeId, Sender<NodeCommand>>,
    pub cli_ser_recv: Receiver<NodeEvent>,
}

impl SimulationController {
    fn crash_all(&mut self, id: u8) {
        if let Some(res) = self.drones.get(&id) {
            match res.send(DroneCommand::Crash) {
                Ok(_) => {
                    println!("Crashead");
                }
                Err(_) => {
                    println!("Crashead");
                }
            }
            for s in self.droness.get(&id).unwrap() {
                println!("{}", s);
                match self
                    .drones
                    .get(s)
                    .unwrap()
                    .send(DroneCommand::RemoveSender(id))
                {
                    Ok(p) => {
                        println!("Sent remove_id to {}", s);
                    }
                    Err(_e) => {
                        println!("remove_id to {} error", s);
                    }
                }
            }
        } else {
            println!("Drone does not exist");
        }
    }
}
