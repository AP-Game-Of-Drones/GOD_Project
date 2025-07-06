use std::collections::{HashMap, HashSet};
use bevy::log::info;
use wg_2024::{network::NodeId, packet::NodeType};

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Node {
    value: NodeId,
    node_type: NodeType,
    pub adjacents: Vec<(NodeId, NodeType)>,
}

impl Node {
    pub fn new(value: NodeId, node_type: NodeType) -> Self {
        Self {
            value,
            node_type,
            adjacents: Vec::new(),
        }
    }

    pub fn add_adjacents(&mut self, id: NodeId, node_type: NodeType) {
        if !self.adjacents.contains(&(id, node_type)) {
            self.adjacents.push((id, node_type));
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Topology {
    nodes: HashMap<NodeId, Node>,
    paths: Option<Vec<(Vec<NodeId>, u64)>>, // Stores paths and weights
    current_path: Option<(Vec<NodeId>, u64)>, // Stores the selected path and weight
}

impl Topology {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            paths: None,
            current_path: None,
        }
    }

    pub fn get_all_servers(&self) -> Vec<NodeId> {
        self.nodes
            .values()
            .filter(|node| node.node_type == NodeType::Server)
            .map(|node| node.value)
            .collect()
    }

    pub fn get_all_clients(&self) -> Vec<NodeId> {
        self.nodes
            .values()
            .filter(|node| node.node_type == NodeType::Client)
            .map(|node| node.value)
            .collect()
    }

    pub fn update_topology(
        &mut self,
        initiator: (NodeId, NodeType),
        mut path_trace: Vec<(NodeId, NodeType)>,
    ) {
        if !path_trace.contains(&initiator) {
            path_trace.insert(0, initiator);
        }

        let len = path_trace.len();
        for (i, &(node_id, node_type)) in path_trace.iter().enumerate() {
            let node = self
                .nodes
                .entry(node_id)
                .or_insert_with(|| Node::new(node_id, node_type));

            if i > 0 {
                node.add_adjacents(path_trace[i - 1].0, path_trace[i - 1].1);
            }
            if i < len - 1 {
                node.add_adjacents(path_trace[i + 1].0, path_trace[i + 1].1);
            }
        }
    }

    pub fn find_all_paths(&mut self, src: NodeId, dst: NodeId) {
        let mut visited = HashSet::new();
        let mut current_path = Vec::new();
        let mut all_paths = Vec::new();

        self.dfs(src, dst, &mut visited, &mut current_path, &mut all_paths, src);

        if all_paths.is_empty() {
            println!("⚠️ Warning: No paths found from {:?} to {:?}", src, dst);
        }
        all_paths.reverse();
        if self.paths.is_none(){
            self.paths = Some(
                all_paths
                .into_iter()
                .map(|path| (path.clone(), path.len() as u64))
                .collect(),
            );
        } else {
            if let Some(paths) = &mut self.paths {
                for new_path in all_paths {
                    let already_exists = paths.iter().any(|(existing_path, _)| *existing_path == new_path);
                    if already_exists {
                        // info!("Path already discovered");
                    } else {
                        paths.push((new_path.clone(), new_path.len() as u64));
                    }
                }
            }
        }
    }

    fn dfs(
        &self,
        current: NodeId,
        dst: NodeId,
        visited: &mut HashSet<NodeId>,
        current_path: &mut Vec<NodeId>,
        all_paths: &mut Vec<Vec<NodeId>>,
        src: NodeId,
    ) {
        if !visited.insert(current) {
            return;
        }
        current_path.push(current);

        if current == dst {
            all_paths.push(current_path.clone());
        } else if let Some(node) = self.nodes.get(&current) {
            for &(neighbor_id, nt) in &node.adjacents {
                if !visited.contains(&neighbor_id) {
                    if nt != NodeType::Drone && dst==neighbor_id {
                        self.dfs(neighbor_id, dst, visited, current_path, all_paths,src);
                    } 
                    if let NodeType::Drone = nt{
                        self.dfs(neighbor_id, dst, visited, current_path, all_paths,src);
                    }
                }
            }
        }

        current_path.pop();
        visited.remove(&current);
    }

    pub fn update_current_path(&mut self) {
        if let Some(paths) = &self.paths {
            if let Some((shortest_path, weight)) = paths.first() {
                self.current_path = Some((shortest_path.clone(), *weight));
            }
        }
    }

    pub fn set_path_based_on_dst(&mut self, dst: NodeId) {
        if let Some(paths) = &self.paths {
            let best_path = paths
                .iter()
                .filter(|(path, _)| path.last() == Some(&dst))
                .min_by_key(|(_, weight)| weight)
                .cloned();

            self.current_path = best_path.clone();

        } else {
            println!("⚠️ No paths available to select for dst: {:?}", dst);
        }
    }

    pub fn get_current_path(&self) -> Option<(Vec<NodeId>,u64)> {
        info!("Current Path {:?}\n{:?}",self.current_path.clone(),self.paths);
        self.current_path.clone()
    }

    pub fn increment_weights_for_node(&mut self, node_id: NodeId) {
        
        if let Some((current_path, weight)) = &mut self.current_path {
            if current_path.contains(&node_id) {
                *weight += 1;
            }
        }

        if let Some(paths) = &mut self.paths {
            for (_, weight) in paths.iter_mut() {
                if *weight < 100000 { // upper bound for a path to not increase anymore
                    *weight += 1;
                }
            }
        }
    }

    pub fn decrease_weights_for_node(&mut self, node_id: NodeId) {

        if let Some((current_path, weight)) = &mut self.current_path {
            if current_path.contains(&node_id) {
                if *weight > 0 {
                    *weight -= 1;
                }
            }
        }

        if let Some(paths) = &mut self.paths {
            for (_, weight) in paths.iter_mut() {
                if *weight > 0 {
                    *weight -= 1;
                }
            }
        }
    }

    

    fn add_node(&mut self, node: Node) {
        self.nodes.insert(node.value, node);
    }

    pub fn remove_node(&mut self, node_id: NodeId) {
        if let Some(node) = self.nodes.remove(&node_id) {
            for &(neighbor_id, _) in &node.adjacents {
                if let Some(neighbor) = self.nodes.get_mut(&neighbor_id) {
                    neighbor.adjacents.retain(|&(id, _)| id != node_id);
                }
            }

            if let Some(paths) = &mut self.paths {
                paths.retain(|(path, _)| !path.contains(&node_id));
            }
        }
    }

    fn get_neighbors(&self, node_id: NodeId) -> Option<Vec<NodeId>> {
        self.nodes
            .get(&node_id)
            .map(|node| node.adjacents.iter().map(|(id, _)| *id).collect())
    }

    fn find_one_path(&self, src: NodeId, dst: NodeId) -> Option<Vec<NodeId>> {
        let mut visited = HashSet::new();
        let mut path = Vec::new();

        if self.dfs_find_one(src, dst, &mut visited, &mut path) {
            Some(path)
        } else {
            None
        }
    }

    /// Helper DFS function to find a single path from `src` to `dst`
    fn dfs_find_one(
        &self,
        current: NodeId,
        dst: NodeId,
        visited: &mut HashSet<NodeId>,
        path: &mut Vec<NodeId>,
    ) -> bool {
        if !visited.insert(current) {
            return false; // Node already visited, avoid cycles
        }

        path.push(current);

        if current == dst {
            return true; // Found the destination, return success
        }

        if let Some(node) = self.nodes.get(&current) {
            for &(neighbor_id, _) in &node.adjacents {
                if self.dfs_find_one(neighbor_id, dst, visited, path) {
                    return true; // Found a valid path, stop searching
                }
            }
        }

        // Backtrack if no path found
        path.pop();
        visited.remove(&current);
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wg_2024::packet::NodeType;

    #[test]
    fn test_add_node() {
        let mut topology = Topology::new();
        let node = Node::new(1, NodeType::Drone);
        topology.add_node(node.clone());

        assert_eq!(topology.nodes.len(), 1);
        assert_eq!(topology.nodes.get(&1).unwrap().value, 1);
    }

    #[test]
    fn test_remove_node() {
        let mut topology = Topology::new();
        let node = Node::new(1, NodeType::Drone);
        topology.add_node(node.clone());

        // Removing the node
        topology.remove_node(1);

        assert_eq!(topology.nodes.len(), 0);
    }

    #[test]
    fn test_find_all_paths() {
        let mut topology = Topology::new();

        // Setup nodes and adjacencies
        let mut node1 = Node::new(1, NodeType::Drone);
        let mut node2 = Node::new(2, NodeType::Drone);
        node1.add_adjacents(2, NodeType::Drone);
        node2.add_adjacents(1, NodeType::Drone);
        topology.add_node(node1);
        topology.add_node(node2);

        topology.find_all_paths(1, 2);

        let paths = topology.paths.as_ref().unwrap();
        assert_eq!(paths[0].1, 2);
        assert_eq!(paths[0].0, vec![1, 2]);
    }

    #[test]
    fn test_update_current_path() {
        let mut topology = Topology::new();

        // Setup nodes and adjacencies
        let mut node1 = Node::new(1, NodeType::Drone);
        let mut node2 = Node::new(2, NodeType::Drone);
        let mut node3 = Node::new(3, NodeType::Drone);
        node1.add_adjacents(2, NodeType::Drone);
        node1.add_adjacents(3, NodeType::Drone);
        node2.add_adjacents(3, NodeType::Drone);
        node2.add_adjacents(1, NodeType::Drone);
        node3.add_adjacents(2, NodeType::Drone);
        node3.add_adjacents(1, NodeType::Drone);

        topology.add_node(node1);
        topology.add_node(node2);
        topology.add_node(node3);

        topology.find_all_paths(1, 3);
        topology.update_current_path();

        let current_path = topology.current_path.as_ref().unwrap();
        assert_eq!(current_path.0, vec![1, 3]);
        assert_eq!(current_path.1, 2); // Weight should be the length of the path
    }

    // #[test]
    // fn test_increment_weights_for_node() {
    //     let mut topology = Topology::new();

    //     // Setup nodes and adjacencies
    //     let mut node1 = Node::new(1, NodeType::Drone);
    //     let mut node2 = Node::new(2, NodeType::Drone);
    //     node1.add_adjacents(2, NodeType::Drone);
    //     node2.add_adjacents(1, NodeType::Drone);
    //     topology.add_node(node1);
    //     topology.add_node(node2);

    //     topology.find_all_paths(1, 2);

    //     // Increment weight for a node in the path
    //     topology.increment_weights_for_node(1);

    //     let current_path = topology.current_path.as_ref().unwrap();
    //     assert_eq!(current_path.1, 3); // Weight should have increased by 1
    // }

    #[test]
    fn test_get_neighbors() {
        let mut topology = Topology::new();

        // Setup nodes and adjacencies
        let mut node1 = Node::new(1, NodeType::Drone);
        let mut node2 = Node::new(2, NodeType::Drone);
        node1.add_adjacents(2, NodeType::Drone);
        node2.add_adjacents(1, NodeType::Drone);
        topology.add_node(node1);
        topology.add_node(node2);

        let neighbors = topology.get_neighbors(1).unwrap();
        assert_eq!(neighbors, vec![2]);
    }
}
