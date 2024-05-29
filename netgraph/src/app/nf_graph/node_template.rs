use std::borrow::Cow;

use super::NFNodeData;

impl egui_node_graph::NodeTemplateTrait for NFNodeData {
    type NodeData = NFNodeData;

    type DataType = super::DataType;

    type ValueType = super::ValueType;

    type UserState = super::NFGraphState;

    type CategoryType = ();

    fn node_finder_label(&self, user_state: &mut Self::UserState) -> Cow<str> {
        if let NFNodeData::Custom { plugin, id, .. } = self {
            let name = user_state.plugins[plugin][id].display_name.clone();
            Cow::from(name)
        } else {
            Cow::from(format!("{self}"))
        }
    }

    fn node_graph_label(&self, user_state: &mut Self::UserState) -> String {
        self.node_finder_label(user_state).into_owned()
    }

    fn user_data(&self, _user_state: &mut Self::UserState) -> Self::NodeData {
        self.clone()
    }

    fn build_node(
        &self,
        graph: &mut egui_node_graph::Graph<Self::NodeData, Self::DataType, Self::ValueType>,
        user_state: &mut Self::UserState,
        node_id: egui_node_graph::NodeId,
    ) {
        use super::data_type::{
            DataType,
            NFDirection::{Either, Incoming, Outgoing},
            NFFamily::{Inet, IPv4, IPv6},
        };

        if let Self::Custom { plugin, id, .. } = self {
            Self::build_custom_node(graph, user_state, node_id, plugin, id);
            return;
        }

        match self {
            NFNodeData::FileIpList(_)
            | NFNodeData::SourceAddressFilter(_)
            | NFNodeData::DestinationAddressFilter(_)
            | NFNodeData::SourcePortFilter(_)
            | NFNodeData::DestinationPortFilter(_)
            | NFNodeData::ProtocolFilter(_)
            | NFNodeData::FamilySplitter
            | NFNodeData::Drop
            | NFNodeData::InterfaceFilter(_)
            | NFNodeData::SourceNAT(_)
            | NFNodeData::DestinationNAT(_) => {
                graph.add_input_param(
                    node_id,
                    String::new(),
                    DataType::new(Inet, Either),
                    super::ValueType,
                    egui_node_graph::InputParamKind::ConnectionOnly,
                    true,
                );
            }
            NFNodeData::Accept => {
                graph.add_input_param(
                    node_id,
                    "outgoing".into(),
                    DataType::new(Inet, Outgoing),
                    super::ValueType,
                    egui_node_graph::InputParamKind::ConnectionOnly,
                    true,
                );
            }
            NFNodeData::Localhost => {
                graph.add_input_param(
                    node_id,
                    "incoming".into(),
                    DataType::new(Inet, Incoming),
                    super::ValueType,
                    egui_node_graph::InputParamKind::ConnectionOnly,
                    true,
                );
            }

            NFNodeData::Source => {}
            NFNodeData::Custom { .. } => {}
        }

        match self {
            NFNodeData::Source => {
                graph.add_output_param(node_id, "incoming".into(), DataType::new(Inet, Incoming));
            }
            NFNodeData::FileIpList(_)
            | NFNodeData::SourceAddressFilter(_)
            | NFNodeData::DestinationAddressFilter(_)
            | NFNodeData::SourcePortFilter(_)
            | NFNodeData::DestinationPortFilter(_)
            | NFNodeData::InterfaceFilter(_)
            | NFNodeData::ProtocolFilter(_) => {
                graph.add_output_param(node_id, "match".into(), DataType::new(Inet, Either));
                graph.add_output_param(node_id, "non-match".into(), DataType::new(Inet, Either));
            }
            NFNodeData::Localhost => {
                graph.add_output_param(node_id, "outgoing".into(), DataType::new(Inet, Outgoing));
            }
            NFNodeData::FamilySplitter => {
                graph.add_output_param(node_id, "ipv4".into(), DataType::new(IPv4, Either));
                graph.add_output_param(node_id, "ipv6".into(), DataType::new(IPv6, Either));
            }
            NFNodeData::DestinationNAT(_) | NFNodeData::SourceNAT(_) => {
                graph.add_output_param(node_id, "".into(), DataType::new(Inet, Either));
            }
            NFNodeData::Drop => {}
            NFNodeData::Accept => {}
            NFNodeData::Custom { .. } => {}
        }
    }
}

impl NFNodeData {
    fn build_custom_node(
        graph: &mut egui_node_graph::Graph<super::NFNodeData, super::DataType, super::ValueType>,
        user_state: &mut super::NFGraphState,
        node_id: egui_node_graph::NodeId,
        plugin: &str,
        id: &str,
    ) {
        use super::{DataType, ValueType};

        let node = &user_state.plugins[plugin][id];

        graph.add_input_param(
            node_id,
            String::new(),
            DataType::new(node.input.family, node.input.direction),
            ValueType,
            egui_node_graph::InputParamKind::ConnectionOnly,
            false,
        );

        for (name, output) in &node.outputs {
            graph.add_output_param(
                node_id,
                name.clone(),
                DataType::new(output.family, output.direction),
            );
        }
    }
}
