#[derive(Debug, Clone)]
pub enum NodeResponse {
    SelectNode(egui_node_graph::NodeId),
}

impl egui_node_graph::UserResponseTrait for NodeResponse {}
