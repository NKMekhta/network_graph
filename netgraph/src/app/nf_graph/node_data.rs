use std::collections::HashMap;
use std::fmt::Display;
use std::path::PathBuf;

use derive_more::IsVariant;
use eframe::egui::{self, Button, RichText, Widget};
use serde::{Deserialize, Serialize};

use egui_node_graph::{Graph, NodeId, NodeResponse};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub port: u16,
}

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone, Serialize, Deserialize, IsVariant)]
pub enum NFNodeData {
    // intermediate nodes
    FileIpList(Option<PathBuf>),
    SourceAddressFilter(String),
    DestinationAddressFilter(String),
    SourcePortFilter(String),
    DestinationPortFilter(String),
    ProtocolFilter(String),
    FamilySplitter,
    InterfaceFilter(String),
    // terminal nodes
    Source,                 // start incoming
    DestinationNAT(String), // terminal for incoming
    Localhost,              // terminal incoming start outgoing
    SourceNAT(String),      // terminal for outgoing
    Drop,                   // terminal for outgoing
    Accept,                 // terminal for outgoing
    Custom {
        plugin: String,
        id: String,
        data: HashMap<String, String>,
    },
}

impl NFNodeData {
    pub(crate) fn get_id(&self) -> String {
        match self {
            NFNodeData::Source => "core:source".into(),
            NFNodeData::FileIpList(_) => "core:file_ip_list".into(),
            NFNodeData::SourceAddressFilter(_) => "core:source_address_filter".into(),
            NFNodeData::DestinationAddressFilter(_) => "core:destination_address_filter".into(),
            NFNodeData::SourcePortFilter(_) => "core:source_port_filter".into(),
            NFNodeData::DestinationPortFilter(_) => "core:destination_port_filter".into(),
            NFNodeData::ProtocolFilter(_) => "core:protocol_filter".into(),
            NFNodeData::FamilySplitter => "core:family_splitter".into(),
            NFNodeData::SourceNAT(_) => "core:source_nat".into(),
            NFNodeData::DestinationNAT(_) => "core:destination_nat".into(),
            NFNodeData::Drop => "core:drop".into(),
            NFNodeData::Accept => "core:accept".into(),
            NFNodeData::Custom { plugin, id, .. } => plugin.clone() + ":" + id,
            NFNodeData::Localhost => "core:localhost".into(),
            NFNodeData::InterfaceFilter(_) => "core:interface_filter".into(),
        }
    }
}

impl egui_node_graph::NodeDataTrait for NFNodeData {
    type Response = super::NodeResponse;

    type UserState = super::NFGraphState;

    type DataType = super::DataType;

    type ValueType = super::ValueType;

    fn bottom_ui(
        &self,
        ui: &mut egui::Ui,
        node_id: egui_node_graph::NodeId,
        _graph: &egui_node_graph::Graph<Self, Self::DataType, Self::ValueType>,
        user_state: &mut Self::UserState,
    ) -> Vec<NodeResponse<Self::Response, Self>> {
        use super::response::NodeResponse::SelectNode;
        let mut responses = vec![];

        ui.separator();
        match self {
            NFNodeData::Source
            | NFNodeData::Drop
            | NFNodeData::Accept
            | NFNodeData::FamilySplitter
            | NFNodeData::Localhost => return responses,

            NFNodeData::FileIpList(file) => {
                ui.label("Matching list file");
                if let Some(file) = file {
                    ui.label(file.to_string_lossy());
                }
            }
            NFNodeData::SourceAddressFilter(addr) => {
                ui.label("Matching Source Address");
                ui.label(addr);
            }
            NFNodeData::DestinationAddressFilter(addr) => {
                ui.label("Matching Destination Address");
                ui.label(addr);
            }
            NFNodeData::SourcePortFilter(ports) => {
                ui.label("Matching Source Port");
                ui.label(ports);
            }
            NFNodeData::DestinationPortFilter(ports) => {
                ui.label("Matching Destination Port");
                ui.label(ports);
            }
            NFNodeData::ProtocolFilter(protocols) => {
                ui.label("Matching Protocol");
                ui.label(protocols);
            }
            NFNodeData::InterfaceFilter(interface) => {
                ui.label("Matching Interface");
                ui.label(interface);
            }
            NFNodeData::SourceNAT(addr) => {
                ui.label("Send from");
                ui.label(addr);
            }
            NFNodeData::DestinationNAT(addr) => {
                ui.label("Send to");
                ui.label(addr);
            }
            NFNodeData::Custom { plugin, id, data } => {
                ui.label(format!("{}", user_state.plugins[plugin][id]));
                for (id, param) in &user_state.plugins[plugin][id].params {
                    let val = data.get(id).cloned().unwrap_or(String::new());
                    ui.label(format!("{param}: {val}"));
                    ui.separator();
                }
            }
        }

        let is_active = user_state.active_node.is_some_and(|id| id == node_id);
        if is_active {
            Button::new(RichText::new("Edit").color(egui::Color32::BLACK))
                .fill(egui::Color32::GOLD)
                .ui(ui);
        } else if ui.button("Edit").clicked() {
            responses.push(NodeResponse::User(SelectNode(node_id)));
        }
        responses
    }

    fn can_delete(
        &self,
        _node_id: NodeId,
        _graph: &Graph<Self, Self::DataType, Self::ValueType>,
        _user_state: &mut Self::UserState,
    ) -> bool {
        match self {
            NFNodeData::Source | NFNodeData::Localhost => false,
            _ => true,
        }
    }
}

impl Display for NFNodeData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NFNodeData::FileIpList(_) => write!(f, "IP File Filter"),
            NFNodeData::SourceAddressFilter(_) => write!(f, "Source Address Filter"),
            NFNodeData::DestinationAddressFilter(_) => write!(f, "Destination Address Filter"),
            NFNodeData::SourcePortFilter(_) => write!(f, "Source Port Filter"),
            NFNodeData::DestinationPortFilter(_) => write!(f, "Destination Port Filter"),
            NFNodeData::ProtocolFilter(_) => write!(f, "Protocol Filter"),
            NFNodeData::FamilySplitter => write!(f, "Family Splitter"),
            NFNodeData::SourceNAT(_) => write!(f, "Source Address Translation"),
            NFNodeData::DestinationNAT(_) => write!(f, "Destination Address Translation"),
            NFNodeData::InterfaceFilter(_) => write!(f, "Interface Filter"),
            NFNodeData::Source => write!(f, "Incoming Source"),
            NFNodeData::Localhost => write!(f, "Local Machine"),
            NFNodeData::Drop => write!(f, "Drop"),
            NFNodeData::Accept => write!(f, "Accept"),
            NFNodeData::Custom { .. } => Err(std::fmt::Error),
        }
    }
}
