use std::{collections::{HashMap, HashSet}, u64::MAX};
use wg_2024::{network::NodeId, packet::NodeType};

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Node {
    value: NodeId,
    node_type: NodeType,
    // pdr: f32, //only if the type is drone.
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
    paths: Option<(Vec<Vec<NodeId>>, u64)>, // Updated to include weight
    current_path: Option<(Vec<NodeId>, u64)>, // Current path and its weight
}

impl Topology {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            paths: None,
            current_path: None,
        }
    }
    pub fn get_all_servers(&self)->Vec<NodeId>{
        self.nodes
        .values()
        .filter(|node| node.node_type == NodeType::Server)
        .map(|node| node.value)
        .collect()
    }

    pub fn update_topology(
        &mut self,
        initiator: (NodeId, NodeType),
        mut path_trace: Vec<(NodeId, NodeType)>,
    ) {
        let mut path_trace_init = Vec::new();
        if !path_trace.contains(&initiator) {
            path_trace_init.push(initiator);
            path_trace_init.append(&mut path_trace);
            path_trace.append(&mut path_trace_init);
        }
        let len = path_trace.len() - 1;
        for value in 0..len + 1 {
            if let Some(node) = self.nodes.get_mut(&path_trace[value].0) {
                if value != len {
                    node.add_adjacents(path_trace[value + 1].0, path_trace[value + 1].1);
                }
                if value != 0 {
                    node.add_adjacents(path_trace[value - 1].0, path_trace[value - 1].1);
                }
            } else {
                let mut node = Node::new(path_trace[value].0, path_trace[value].1);
                if value != len {
                    node.add_adjacents(path_trace[value + 1].0, path_trace[value + 1].1);
                }
                if value != 0 {
                    node.add_adjacents(path_trace[value - 1].0, path_trace[value - 1].1);
                }
                self.nodes.insert(node.value, node);
            }
        }
    }

    pub fn find_all_paths(&mut self, src: NodeId, dst: NodeId) {
        self.paths = None; // Clear existing paths
        let mut visited = HashSet::new();
        let mut current_path = Vec::new();
        let mut all_paths = Vec::new();

        // Start DFS traversal from the fixed source node
        self.dfs(src, dst, &mut visited, &mut current_path, &mut all_paths);

        // Calculate weights for all paths (e.g., based on path length)
        let weight = all_paths.iter().map(|path| path.len() as u64).sum();

        // Sort paths by length
        all_paths.sort_by_key(|path| path.len());
        self.paths = Some((all_paths, weight));
    }

    /// Helper function to perform Depth-First Search
    fn dfs(
        &self,
        current: NodeId,
        dst: NodeId,
        visited: &mut HashSet<NodeId>,
        current_path: &mut Vec<NodeId>,
        all_paths: &mut Vec<Vec<NodeId>>,
    ) {
        visited.insert(current);
        current_path.push(current);

        if current == dst {
            all_paths.push(current_path.clone()); // Add the path if the destination is reached
        } else if let Some(node) = self.nodes.get(&current) {
            for &(neighbor_id, _) in &node.adjacents {
                if !visited.contains(&neighbor_id) {
                    self.dfs(neighbor_id, dst, visited, current_path, all_paths);
                }
            }
        }

        // Backtrack
        visited.remove(&current);
        current_path.pop();
    }

    /// Updates the `current_path` by selecting one from the available paths based on length and weight
    pub fn update_current_path(&mut self) {
        if let Some((paths, _)) = &self.paths {
            if let Some(shortest_path) = paths.first() {
                // For simplicity, set the weight as the path length
                let weight = shortest_path.len() as u64;
                self.current_path = Some((shortest_path.clone(), weight));
            }
        }
    }

    pub fn set_path_based_on_dst(&mut self, dst: NodeId) {
        if let Some((paths, w )) = &self.paths {
            let mut current = MAX;
            for path in paths {
                if path[path.clone().len()-1]==dst{
                    if *w<current {
                        self.current_path=Some((path.clone(),*w));
                        current=*w;
                    }
                }
            }
        }
    }

    pub fn get_current_path(&self)->Vec<u8> {
        self.current_path.clone().unwrap().0
    }

    pub fn increment_weights_for_node(&mut self, node_id: NodeId) {
        // Update the weight of the `current_path`
        if let Some((current_path, weight)) = &mut self.current_path {
            if current_path.contains(&node_id) {
                *weight += 1;
            }
        }

        // Update the weights of all paths
        if let Some((paths, total_weight)) = &mut self.paths {
            for path in paths {
                if path.contains(&node_id) {
                    *total_weight += 1;
                }
            }
        }
    }

    pub fn add_node(&mut self, node: Node) {
        self.nodes.insert(node.value, node);
    }

    pub fn remove_node(&mut self, node_id: NodeId) {
        // Remove the node from the topology
        if let Some(node) = self.nodes.remove(&node_id) {
            // Remove the node from the adjacencies of its neighbors
            for &(neighbor_id, _) in &node.adjacents {
                if let Some(neighbor) = self.nodes.get_mut(&neighbor_id) {
                    // Remove the removed node from each neighbor's adjacency list
                    neighbor.adjacents.retain(|&(id, _)| id != node_id);
                }
            }

            // Now that the node is removed, we need to remove it from all paths that contain it
            if let Some((paths, _)) = &mut self.paths {
                // Retain paths that do not contain the node_id
                paths.retain(|path| !path.contains(&node_id));
            }

            // Call `find_all_paths` to recompute the paths after removal
            // For recomputation, you should pass the correct source and destination, here assuming node_id is both.
            self.find_all_paths(node_id, node_id);
        }
    }

    pub fn get_neighbors(&self, node_id: NodeId) -> Option<Vec<NodeId>> {
        if let Some(node) = self.nodes.get(&node_id) {
            // Collecting the NodeIds of the neighbors
            let neighbors: Vec<NodeId> = node
                .adjacents
                .iter()
                .map(|(neighbor_id, _)| *neighbor_id)
                .collect();
            Some(neighbors)
        } else {
            // If the node doesn't exist in the topology, return None
            None
        }
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

        let paths = topology.paths.as_ref().unwrap().0.clone();
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0], vec![1, 2]);
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

    #[test]
    fn test_increment_weights_for_node() {
        let mut topology = Topology::new();

        // Setup nodes and adjacencies
        let mut node1 = Node::new(1, NodeType::Drone);
        let mut node2 = Node::new(2, NodeType::Drone);
        node1.add_adjacents(2, NodeType::Drone);
        node2.add_adjacents(1, NodeType::Drone);
        topology.add_node(node1);
        topology.add_node(node2);

        topology.find_all_paths(1, 2);
        topology.update_current_path();

        // Increment weight for a node in the path
        topology.increment_weights_for_node(1);

        let current_path = topology.current_path.as_ref().unwrap();
        assert_eq!(current_path.1, 3); // Weight should have increased by 1
    }

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
