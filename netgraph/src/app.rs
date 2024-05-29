use std::{collections::HashMap, path::PathBuf};

use eframe::egui;
use eframe::egui::Widget;
use serde::{Deserialize, Serialize};

use egui_node_graph::{GraphEditorState, InputId, NodeId, NodeTemplateTrait, OutputId};
use nf_graph::{DataType, NFGraphState, NFNodeData as NodeData, NodeTemplateIter, ValueType};

use crate::app::nf_graph::{NFDirection, NFNodeData};

mod nf_graph;
mod plugin;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Predicate {
    variant: String,
    params: HashMap<String, String>,
}
type PredicateSet = Vec<Predicate>;
type NodeOutputs = HashMap<String, Vec<PredicateSet>>;
type NodeOutputDB = HashMap<NodeId, NodeOutputs>;

pub struct App {
    editor_state: GraphEditorState<NodeData, DataType, ValueType, NodeData, NFGraphState>,
    user_state: NFGraphState,
    source_node: NodeId,
    all_kinds: NodeTemplateIter,
}

pub enum PathNode {
    Source {
        node_id: NodeId,
        output_id: OutputId,
    },
    Destination {
        node_id: NodeId,
        input_id: InputId,
    },
    Intermediate {
        node_id: NodeId,
        input_id: InputId,
        output_id: OutputId,
    },
}

impl Default for App {
    fn default() -> Self {
        let mut slf = Self {
            editor_state: GraphEditorState::default(),
            user_state: NFGraphState {
                active_node: None,
                plugins: HashMap::new(),
            },
            source_node: Default::default(),
            all_kinds: NodeTemplateIter::new(Vec::new()),
        };
        slf.new_graph();
        slf
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                egui::widgets::global_dark_light_mode_switch(ui);
            });
        });

        egui::SidePanel::right("side_panel")
            .min_width(200.0)
            .show(ctx, |ui| {
                if let Some(node_id) = self.user_state.active_node {
                    let node = &mut self.editor_state.graph.nodes[node_id].user_data;
                    let plugins = &self.user_state.plugins;
                    ui.label(format!("ID: {}", node.get_id()));
                    ui.label(format!("{node}"));

                    match node {
                        NFNodeData::Custom { plugin, id, data } => {
                            let params: &HashMap<String, String> = &plugins[plugin][id].params;
                            for (param_id, param_name) in params {
                                ui.label(param_name);
                                if !data.contains_key(param_id) {
                                    data.insert(param_id.clone(), String::new());
                                }
                                egui::TextEdit::singleline(data.get_mut(param_id).unwrap()).ui(ui);
                            }
                        }
                        NodeData::FileIpList(path) => {
                            if ui.button("Open file…").clicked() {
                                if let Some(new_path) = rfd::FileDialog::new().pick_file() {
                                    *path = Some(PathBuf::from(new_path.display().to_string()));
                                }
                            }
                            if let Some(picked_path) = &path {
                                ui.horizontal(|ui| {
                                    ui.label("Picked file:");
                                    ui.monospace(picked_path.to_string_lossy());
                                });
                            }
                        }
                        NodeData::SourceAddressFilter(filter) => {
                            ui.label("Match source address:");
                            egui::TextEdit::singleline(filter).ui(ui);
                        }
                        NFNodeData::DestinationAddressFilter(filter) => {
                            ui.label("Match destination address:");
                            egui::TextEdit::singleline(filter).ui(ui);
                        }
                        NFNodeData::SourcePortFilter(port) => {
                            ui.label("Match source port:");
                            egui::TextEdit::singleline(port).ui(ui);
                        }
                        NFNodeData::DestinationPortFilter(port) => {
                            ui.label("Match destination port:");
                            egui::TextEdit::singleline(port).ui(ui);
                        }
                        NFNodeData::ProtocolFilter(protocol) => {
                            ui.label("Match protocol:");
                            egui::TextEdit::singleline(protocol).ui(ui);
                        }
                        NFNodeData::InterfaceFilter(ifname) => {
                            ui.label("Match interface:");
                            egui::TextEdit::singleline(ifname).ui(ui);
                        }
                        NFNodeData::DestinationNAT(addr) => {
                            ui.label("Direct packet to:");
                            egui::TextEdit::singleline(addr).ui(ui);
                        }
                        NFNodeData::SourceNAT(addr) => {
                            ui.label("Send packet from:");
                            egui::TextEdit::singleline(addr).ui(ui);
                        }
                        NFNodeData::Localhost => {}
                        NFNodeData::Drop => {}
                        NFNodeData::Accept => {}
                        NFNodeData::Source => {}
                        NFNodeData::FamilySplitter => {}
                    }
                    return;
                }

                if ui.button("Import an extension").clicked() {
                    self.reload_all_kinds();
                }

                if ui.button("Export configuration").clicked() {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&self.collect_paths().unwrap()).unwrap()
                    );
                };

                if ui.button("New node graph").clicked() {
                    self.new_graph();
                }

                ui.button("Save node graph");
                ui.button("Load node graph");
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            use egui_node_graph::NodeResponse::{ConnectEventEnded, DeleteNodeFull, User};
            use nf_graph::NodeResponse::SelectNode;

            let responses = self.editor_state.draw_graph_editor(
                ui,
                self.all_kinds.clone(),
                &mut self.user_state,
                vec![],
            );
            for response in responses.node_responses {
                match response {
                    User(SelectNode(node_id)) => self.user_state.active_node = Some(node_id),
                    ConnectEventEnded {
                        input: input_id,
                        output: output_id,
                    } => {
                        if self.break_loops(output_id).is_some() {
                            self.editor_state.graph.connections.remove(output_id);
                        }
                        self.propagate_data_types(input_id, output_id);
                    }
                    DeleteNodeFull { node_id, .. } => {
                        if self.user_state.active_node == Some(node_id) {
                            self.user_state.active_node = None;
                        }
                    }
                    _ => {}
                }
            }
        });
    }
}

impl App {
    fn reload_all_kinds(&mut self) {
        let custom_kinds: Vec<NodeData> = self
            .user_state
            .plugins
            .iter()
            .flat_map(|(plugin_id, node_map)| {
                node_map.iter().map(|(node_id, ..)| NodeData::Custom {
                    plugin: plugin_id.clone(),
                    id: node_id.clone(),
                    data: HashMap::new(),
                })
            })
            .collect();
        self.all_kinds = NodeTemplateIter::new(custom_kinds);
    }

    fn propagate_data_types(&mut self, input_id: InputId, output_id: OutputId) -> Option<()> {
        let input = self.editor_state.graph.inputs.get(input_id)?;
        let output = self.editor_state.graph.outputs.get(output_id)?;
        if output.typ.direction == NFDirection::Either {
            return Some(());
        }
        if input.typ.direction != NFDirection::Either {
            return Some(());
        }
        self.propagate_to_node(input_id, output.typ.direction)
    }

    fn propagate_to_node(&mut self, input_id: InputId, direction: NFDirection) -> Option<()> {
        self.editor_state
            .graph
            .inputs
            .get_mut(input_id)?
            .typ
            .direction = direction;
        let input = self.editor_state.graph.inputs.get(input_id)?;
        let node = self.editor_state.graph.nodes.get(input.node)?;
        for (_, output_id) in node.outputs.clone() {
            self.editor_state
                .graph
                .outputs
                .get_mut(output_id)?
                .typ
                .direction = direction;
            if let Some(connected_input_id) = self.editor_state.graph.connections.get(output_id) {
                self.propagate_to_node(*connected_input_id, direction);
            }
        }
        Some(())
    }

    fn new_graph(&mut self) {
        self.editor_state = GraphEditorState::default();
        self.user_state = NFGraphState::default();
        self.all_kinds = NodeTemplateIter::new(Vec::new());

        for node_template in [NFNodeData::Source, NFNodeData::Localhost] {
            let node = self.editor_state.graph.add_node(
                node_template.node_graph_label(&mut self.user_state),
                node_template.user_data(&mut self.user_state),
                |graph, node_id| node_template.build_node(graph, &mut self.user_state, node_id),
            );
            self.editor_state
                .node_positions
                .insert(node, Default::default());
            self.editor_state.node_order.push(node);
        }
    }

    fn get_receiving_node(&self, output_id: OutputId) -> Option<NodeId> {
        let connected_input = self.editor_state.graph.connections.get(output_id)?;
        let connected_input_node = self.editor_state.graph.inputs.get(*connected_input)?;
        Some(connected_input_node.node)
    }

    fn get_sending_nodes(&self, input_id: InputId) -> Vec<NodeId> {
        self.editor_state
            .graph
            .connections
            .iter()
            .filter(|(_, &iid)| iid == input_id)
            .map(|(oid, _)| oid)
            .filter_map(|oid| Some(self.editor_state.graph.outputs.get(oid)?.node))
            .collect()
    }

    fn get_connected_receiver_nodes(&self, node_id: NodeId) -> Vec<NodeId> {
        let Some(node) = self.editor_state.graph.nodes.get(node_id) else {
            return vec![];
        };
        node.outputs
            .iter()
            .filter_map(|(_, output_id)| self.get_receiving_node(*output_id))
            .collect()
    }

    fn get_connected_sender_nodes(&self, node_id: NodeId) -> Vec<NodeId> {
        let Some(node) = self.editor_state.graph.nodes.get(node_id) else {
            return vec![];
        };
        node.inputs
            .iter()
            .map(|(_, input_id)| self.get_sending_nodes(*input_id))
            .flatten()
            .collect()
    }

    fn apply_custom_node(
        &self,
        input: &PredicateSet,
        node_data: &NodeData,
        output_name: &str,
    ) -> anyhow::Result<PredicateSet> {
        todo!("Custom node to predicate")
    }

    fn apply_node(
        &self,
        input: &PredicateSet,
        node_data: &NodeData,
        output_name: &str,
    ) -> anyhow::Result<PredicateSet> {
        let id = node_data.get_id().clone();
        match node_data {
            NodeData::Custom { .. } => self.apply_custom_node(input, node_data, output_name),

            NFNodeData::FileIpList(path) => {
                let path = match path {
                    Some(path) => path,
                    None => return Err(anyhow::anyhow!("IP List file is required")),
                };
                let predicate = match output_name {
                    "match" => Predicate {
                        variant: id.clone(),
                        params: HashMap::from([("allow".into(), path.to_string_lossy().into())]),
                    },
                    "non-match" => Predicate {
                        variant: id,
                        params: HashMap::from([("exclude".into(), path.to_string_lossy().into())]),
                    },
                    _ => return Err(anyhow::anyhow!("Unknown output name")),
                };
                Ok([input.clone(), vec![predicate]].concat())
            }

            NFNodeData::SourceAddressFilter(filter)
            | NFNodeData::DestinationAddressFilter(filter)
            | NFNodeData::SourcePortFilter(filter)
            | NFNodeData::DestinationPortFilter(filter)
            | NFNodeData::InterfaceFilter(filter)
            | NFNodeData::ProtocolFilter(filter) => {
                let predicate = match output_name {
                    "match" => Predicate {
                        variant: id.clone(),
                        params: HashMap::from([("allow".into(), filter.clone())]),
                    },
                    "non-match" => Predicate {
                        variant: id,
                        params: HashMap::from([("exclude".into(), filter.clone())]),
                    },
                    _ => return Err(anyhow::anyhow!("Unknown output name")),
                };
                Ok([input.clone(), vec![predicate]].concat())
            }

            NodeData::SourceNAT(addr) | NodeData::DestinationNAT(addr) => {
                let predicate = Predicate {
                    variant: id.clone(),
                    params: HashMap::from([("addr".into(), addr.clone())]),
                };
                Ok([input.clone(), vec![predicate]].concat())
            }
            NodeData::Localhost => {
                let predicate = Predicate {
                    variant: id.clone(),
                    params: HashMap::new(),
                };
                Ok([input.clone(), vec![predicate]].concat())
            }
            NodeData::Drop => {
                let predicate = Predicate {
                    variant: id.clone(),
                    params: HashMap::new(),
                };
                Ok([input.clone(), vec![predicate]].concat())
            }
            NodeData::Accept => {
                let predicate = Predicate {
                    variant: id.clone(),
                    params: HashMap::new(),
                };
                Ok([input.clone(), vec![predicate]].concat())
            }
            NFNodeData::FamilySplitter => {
                let predicate = match output_name {
                    "ipv4" => Predicate {
                        variant: id.clone(),
                        params: HashMap::from([("family".into(), "ipv4".into())]),
                    },
                    "ipv6" => Predicate {
                        variant: id.clone(),
                        params: HashMap::from([("family".into(), "ipv6".into())]),
                    },
                    _ => return Err(anyhow::anyhow!("Unknown output name")),
                };
                Ok([input.clone(), vec![predicate]].concat())
            }
            NFNodeData::Source => Ok(vec![Predicate {
                variant: id.clone(),
                params: HashMap::new(),
            }]),
        }
    }

    fn node_outputs(
        &self,
        node_inputs: &Vec<PredicateSet>,
        node_id: NodeId,
    ) -> anyhow::Result<NodeOutputs, anyhow::Error> {
        let mut output_predicates = HashMap::new();
        let node = self
            .editor_state
            .graph
            .nodes
            .get(node_id)
            .ok_or(anyhow::Error::msg("Node not found"))?;
        let node_data = &node.user_data;
        let mut outputs = node.outputs.clone();

        if outputs.is_empty() {
            outputs.push(("terminal".into(), OutputId::default()));
        }
        for (output_name, _) in outputs {
            let mut node_output = Vec::new();
            for input in node_inputs {
                node_output.push(self.apply_node(input, node_data, &output_name)?);
            }
            // let output: Vec<_> = inputs
            //     .clone()
            //     .iter()
            //     .map(|input| self.apply_node(input, node_data, output_name))
            //     .collect();
            output_predicates.insert(output_name.clone(), node_output);
        }

        Ok(output_predicates)
    }

    fn recurse_node_outputs(
        &self,
        this_node_id: &NodeId,
        node_output_db: &mut NodeOutputDB,
    ) -> anyhow::Result<()> {
        if node_output_db.contains_key(this_node_id) {
            return Ok(());
        }
        let this_node = self
            .editor_state
            .graph
            .nodes
            .get(*this_node_id)
            .ok_or(anyhow::Error::msg("Node not found"))?;
        let Some((_, this_node_input_id)) = this_node.inputs.get(0) else {
            node_output_db.insert(
                *this_node_id,
                self.node_outputs(&vec![PredicateSet::new()], *this_node_id)?,
            );
            return Ok(());
        };
        let this_node_inputs: Vec<PredicateSet> = self
            .get_connected_sender_nodes(*this_node_id)
            .iter()
            .filter_map(|dep_node_id| {
                self.recurse_node_outputs(&dep_node_id, node_output_db)
                    .ok()?;
                let dep_node = &self.editor_state.graph.nodes.get(*dep_node_id)?;

                let dep_node_outputs: Vec<_> = dep_node
                    .outputs
                    .iter()
                    .filter_map(|(output_name, output_id)| {
                        if *this_node_input_id
                            == *self.editor_state.graph.connections.get(*output_id)?
                        {
                            Some(node_output_db.get(dep_node_id)?.get(output_name)?.clone())
                        } else {
                            None
                        }
                    })
                    .collect();
                Some(dep_node_outputs)
            })
            .flatten()
            .flatten()
            .collect();

        node_output_db.insert(
            this_node_id.clone(),
            self.node_outputs(&this_node_inputs, *this_node_id)?,
        );
        Ok(())
    }

    fn collect_paths(&self) -> anyhow::Result<Vec<PredicateSet>> {
        let mut node_output_db = NodeOutputDB::new();
        for node_id in self.editor_state.graph.iter_nodes() {
            self.recurse_node_outputs(&node_id, &mut node_output_db)?;
        }
        let terminal_nodes = self
            .editor_state
            .graph
            .nodes
            .iter()
            .filter(|(_, node)| node.outputs.is_empty())
            .filter_map(|(node_id, _)| Some(node_output_db.get(&node_id)?.get("terminal")?.clone()))
            .flatten()
            .collect();
        Ok(terminal_nodes)
    }

    fn break_loops(&self, output_id: OutputId) -> Option<()> {
        let root_node_id = self.editor_state.graph.outputs.get(output_id)?.node;
        let mut stack = vec![(0usize, self.get_connected_receiver_nodes(root_node_id))];

        loop {
            let traverse_deeper = {
                let (subtree_index, subtree_nodes) = stack.last()?;
                if let Some(node_id) = subtree_nodes.get(*subtree_index) {
                    if *node_id == root_node_id {
                        return Some(());
                    }
                    true
                } else {
                    false
                }
            };

            if traverse_deeper {
                stack.last_mut()?.0 += 1;
                let node_id = stack.last()?.1.get(stack.last()?.0 - 1)?.clone();
                stack.push((0, self.get_connected_receiver_nodes(node_id)));
            } else {
                stack.pop();
            }
        }
    }
}
