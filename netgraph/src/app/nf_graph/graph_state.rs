use std::collections::HashMap;

use egui_node_graph::NodeId;

use crate::app::plugin::NFNode;

#[derive(Debug, Clone, Default)]
pub struct NFGraphState {
    pub plugins: HashMap<String, HashMap<String, NFNode>>,
    pub active_node: Option<NodeId>,
}
