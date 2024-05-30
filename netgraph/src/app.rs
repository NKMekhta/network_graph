use std::{collections::HashMap, path::PathBuf};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::{Read, Write};

use eframe::egui;
use eframe::egui::{Pos2, Widget};
use egui_notify::Anchor;
use map_macro::hash_map;
use nftables::expr::Expression;
use nftables::schema::{NfCmd, NfListObject, NfObject, Nftables};
use nftables::stmt::NATFamily;
use serde::{Deserialize, Serialize};
use serde_json::Map;
use tap::Tap;

use egui_node_graph::{GraphEditorState, InputId, NodeId, NodeTemplateTrait, OutputId};
use nf_graph::{DataType, NFGraphState, NFNodeData as NodeData, NodeTemplateIter, ValueType};

use crate::app::nf_graph::{NFDirection, NFNodeData};
use crate::app::plugin::Plugin;

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
    current_graph_path: Option<PathBuf>,
    toasts: egui_notify::Toasts,
}

impl Default for App {
    fn default() -> Self {
        let mut slf = Self {
            editor_state: GraphEditorState::default(),
            user_state: NFGraphState {
                active_node: None,
                plugins: HashMap::new(),
            },
            source_node: NodeId::default(),
            all_kinds: NodeTemplateIter::new(Vec::new()),
            current_graph_path: None,
            toasts: egui_notify::Toasts::new().with_anchor(Anchor::BottomRight),
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
                    if let NFNodeData::Custom { plugin, id, .. } = &node {
                        ui.label(format!("{}", plugins[plugin][id].display_name));
                    } else {
                        ui.label(format!("{node}"));
                    }

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
                        NFNodeData::Localhost
                        | NFNodeData::Accept
                        | NFNodeData::Drop
                        | NFNodeData::Source
                        | NFNodeData::FamilySplitter => {}
                    }
                    return;
                }

                if ui.button("Import an extension").clicked() {
                    match self.import_extension() {
                        Ok(()) => self.toasts.success("Extension imported successfully"),
                        Err(err) => self.toasts.error(err.to_string()),
                    };
                }

                if ui.button("Export configuration").clicked() {
                    match self.export_configuration() {
                        Ok(()) => self.toasts.success("Configuration exported successfully"),
                        Err(err) => self.toasts.error(err.to_string()),
                    };
                };

                if ui.button("New node graph").clicked() {
                    self.new_graph();
                }

                if ui.button("Save node graph").clicked() {
                    match self.save_node_graph() {
                        Ok(()) => self.toasts.success("Graph saved"),
                        Err(err) => self.toasts.error(err.to_string()),
                    };
                }

                if ui.button("Load node graph").clicked() {
                    match self.load_node_graph() {
                        Ok(()) => self.toasts.success("Graph loaded"),
                        Err(err) => self.toasts.error(err.to_string()),
                    };
                }
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
        self.toasts.show(ctx);
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
            .flat_map(|(_, input_id)| self.get_sending_nodes(*input_id))
            .collect()
    }

    fn apply_custom_node(
        &self,
        input: &PredicateSet,
        node_data: &NodeData,
        output_name: &str,
    ) -> anyhow::Result<(PredicateSet, HashMap<String, String>)> {
        #[derive(Serialize, Deserialize)]
        struct OutputData {
            predicate_set: PredicateSet,
            custom_data: HashMap<String, String>,
        }
        let path = self.current_graph_path.clone().unwrap();
        let NFNodeData::Custom { plugin, id, .. } = node_data else {
            return Err(anyhow::anyhow!("Node is not custom"));
        };
        let script_path = path
            .clone()
            .tap_mut(|s| s.as_mut_os_string().push(format!("/plugins/{plugin}/{id}")));
        let mut child = std::process::Command::new(script_path)
            .arg(output_name)
            .spawn()
            .map_err(|e| anyhow::anyhow!("Cannot execute node script: {}", e))?;
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(serde_json::to_string(input)?.as_bytes())?;
        let mut output = vec![];
        child.stdout.as_mut().unwrap().read_to_end(&mut output)?;
        let output = String::from_utf8(output).map_err(|e| anyhow::anyhow!("{}", e))?;
        let output_data: OutputData = serde_json::from_str(&output)
            .map_err(|e| anyhow::anyhow!("Cannot deserialize node script output: {}", e))?;

        Ok((output_data.predicate_set, output_data.custom_data))
    }

    fn apply_node(
        &self,
        input: &PredicateSet,
        node_data: &NodeData,
        output_name: &str,
    ) -> anyhow::Result<PredicateSet> {
        let id = node_data.get_id().clone();
        match node_data {
            NodeData::Custom { .. } => Ok(self.apply_custom_node(input, node_data, output_name)?.0),

            NFNodeData::FileIpList(path) => {
                let Some(path) = path else {
                    return Err(anyhow::anyhow!("IP List file is required"));
                };
                let predicate = Predicate {
                    variant: id.clone(),
                    params: hash_map! {
                        String::from("path") => path.to_string_lossy().to_string(),
                        String::from("rule") => output_name.to_string(),
                    },
                };
                Ok([input.clone(), vec![predicate]].concat())
            }

            NFNodeData::SourceAddressFilter(filter)
            | NFNodeData::DestinationAddressFilter(filter)
            | NFNodeData::SourcePortFilter(filter)
            | NFNodeData::DestinationPortFilter(filter)
            | NFNodeData::InterfaceFilter(filter)
            | NFNodeData::ProtocolFilter(filter) => {
                let predicate = Predicate {
                    variant: id.clone(),
                    params: hash_map! {
                        String::from("filter") => filter.to_string(),
                        String::from("rule") => output_name.to_string(),
                    },
                };
                Ok([input.clone(), vec![predicate]].concat())
            }

            NodeData::SourceNAT(addr) | NodeData::DestinationNAT(addr) => {
                let predicate = Predicate {
                    variant: id.clone(),
                    params: hash_map! {
                        String::from("addr") => addr.clone(),
                    },
                };
                Ok([input.clone(), vec![predicate]].concat())
            }
            NodeData::Localhost | NodeData::Accept | NodeData::Drop => {
                let predicate = Predicate {
                    variant: id.clone(),
                    params: hash_map! {},
                };
                Ok([input.clone(), vec![predicate]].concat())
            }
            NFNodeData::FamilySplitter => {
                let predicate = Predicate {
                    variant: id.clone(),
                    params: hash_map! {
                        String::from("family") => output_name.to_string(),
                    },
                };
                Ok([input.clone(), vec![predicate]].concat())
            }
            NFNodeData::Source => Ok(vec![Predicate {
                variant: id.clone(),
                params: hash_map! {},
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
            .ok_or(anyhow::anyhow!("Node not found"))?;
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
            output_predicates.insert(output_name.clone(), node_output);
        }

        Ok(output_predicates)
    }

    fn recurse_node_outputs(
        &self,
        this_node_id: NodeId,
        node_output_db: &mut NodeOutputDB,
    ) -> anyhow::Result<()> {
        if node_output_db.contains_key(&this_node_id) {
            return Ok(());
        }
        let this_node = self
            .editor_state
            .graph
            .nodes
            .get(this_node_id)
            .ok_or(anyhow::Error::msg("Node not found"))?;
        let Some((_, this_node_input_id)) = this_node.inputs.first() else {
            node_output_db.insert(
                this_node_id,
                self.node_outputs(&vec![PredicateSet::new()], this_node_id)?,
            );
            return Ok(());
        };
        let this_node_inputs: Vec<PredicateSet> = self
            .get_connected_sender_nodes(this_node_id)
            .iter()
            .filter_map(|&dep_node_id| {
                self.recurse_node_outputs(dep_node_id, node_output_db)
                    .ok()?;
                let dep_node = &self.editor_state.graph.nodes.get(dep_node_id)?;

                let dep_node_outputs: Vec<_> = dep_node
                    .outputs
                    .iter()
                    .filter_map(|(output_name, output_id)| {
                        if *this_node_input_id
                            == *self.editor_state.graph.connections.get(*output_id)?
                        {
                            Some(node_output_db.get(&dep_node_id)?.get(output_name)?.clone())
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
            this_node_id,
            self.node_outputs(&this_node_inputs, this_node_id)?,
        );
        Ok(())
    }

    fn evaluate_path(path: &PredicateSet) -> anyhow::Result<Vec<NfObject>> {
        use nf::{
            schema::{Chain, NfObject::CmdObject, Rule},
            stmt::{Match, Operator},
        };
        use nftables as nf;

        let mut hasher = DefaultHasher::new();
        path.hash(&mut hasher);
        let mut chain_name = hasher.finish();
        let mut current_subpath = vec![];
        let mut is_incoming = true;
        let mut objects = vec![];

        for predicate in path {
            let variant = predicate.variant.as_str();
            match variant {
                "core:source_address_filter" => {
                    let rule = predicate
                        .params
                        .get("rule")
                        .ok_or(anyhow::anyhow!("Rule is required"))?;
                    let filter = predicate
                        .params
                        .get("filter")
                        .ok_or(anyhow::anyhow!("Filter is required"))?;
                    let match_rule = Match {
                        left: Expression::String("saddr".into()),
                        right: Expression::String(filter.to_string()),
                        op: if rule == "match" {
                            Operator::EQ
                        } else {
                            Operator::NEQ
                        },
                    };
                    current_subpath.push(match_rule);
                }
                "core:destination_address_filter" => {
                    let rule = predicate
                        .params
                        .get("rule")
                        .ok_or(anyhow::anyhow!("Rule is required"))?;
                    let filter = predicate
                        .params
                        .get("filter")
                        .ok_or(anyhow::anyhow!("Filter is required"))?;
                    let match_rule = Match {
                        left: Expression::String("daddr".into()),
                        right: Expression::String(filter.to_string()),
                        op: if rule == "match" {
                            Operator::EQ
                        } else {
                            Operator::NEQ
                        },
                    };
                    current_subpath.push(match_rule);
                }
                "core:source_port_filter" => {
                    let rule = predicate
                        .params
                        .get("rule")
                        .ok_or(anyhow::anyhow!("Rule is required"))?;
                    let filter = predicate
                        .params
                        .get("filter")
                        .ok_or(anyhow::anyhow!("Filter is required"))?;
                    let match_rule = Match {
                        left: Expression::String("meta l4proto { tcp, udp } th sport".into()),
                        right: Expression::String(filter.into()),
                        op: if filter == rule {
                            Operator::EQ
                        } else {
                            Operator::NEQ
                        },
                    };
                    current_subpath.push(match_rule);
                }
                "core:destination_port_filter" => {
                    let rule = predicate
                        .params
                        .get("rule")
                        .ok_or(anyhow::anyhow!("Rule is required"))?;
                    let filter = predicate
                        .params
                        .get("filter")
                        .ok_or(anyhow::anyhow!("Filter is required"))?;
                    let match_rule = Match {
                        left: Expression::String("meta l4proto { tcp, udp } th dport".into()),
                        right: Expression::String(filter.to_string()),
                        op: if rule == "match" {
                            Operator::EQ
                        } else {
                            Operator::NEQ
                        },
                    };
                    current_subpath.push(match_rule);
                }
                "core:protocol_filter" => {
                    let rule = predicate
                        .params
                        .get("rule")
                        .ok_or(anyhow::anyhow!("Rule is required"))?;
                    let filter = predicate
                        .params
                        .get("filter")
                        .ok_or(anyhow::anyhow!("Filter is required"))?;
                    let match_rule = Match {
                        left: Expression::String("ip protocol".into()),
                        right: Expression::String(filter.to_string()),
                        op: if rule == "match" {
                            Operator::EQ
                        } else {
                            Operator::NEQ
                        },
                    };
                    current_subpath.push(match_rule);
                }
                "core:interface_filter" => {
                    let rule = predicate
                        .params
                        .get("rule")
                        .ok_or(anyhow::anyhow!("Rule is required"))?;
                    let filter = predicate
                        .params
                        .get("filter")
                        .ok_or(anyhow::anyhow!("Filter is required"))?;
                    let interface = if is_incoming { "iifname" } else { "oifname" };
                    let match_rule = Match {
                        left: Expression::String(interface.into()),
                        right: Expression::String(filter.to_string()),
                        op: if rule == "match" {
                            Operator::EQ
                        } else {
                            Operator::NEQ
                        },
                    };
                    current_subpath.push(match_rule);
                }
                "core:family_splitter" => {
                    let family = predicate
                        .params
                        .get("family")
                        .ok_or(anyhow::anyhow!("Family is required"))?;
                    let match_rule = Match {
                        left: Expression::String("nfproto".into()),
                        right: Expression::String(family.to_string()),
                        op: Operator::EQ,
                    };
                    current_subpath.push(match_rule);
                }
                "core:file_ip_list" => {
                    todo!("File IP list rules");
                }
                "core:source_nat" => {
                    let hook = if is_incoming {
                        nf::types::NfHook::Input
                    } else {
                        nf::types::NfHook::Output
                    };
                    let addr = predicate
                        .params
                        .get("addr")
                        .ok_or(anyhow::anyhow!("Address is required"))?;
                    let chain = Chain::new(
                        nf::types::NfFamily::INet,
                        "netgraph".into(),
                        chain_name.clone().to_string(),
                        Some(nf::types::NfChainType::NAT),
                        Some(hook),
                        None,
                        None,
                        Some(nf::types::NfChainPolicy::Accept),
                    );
                    let rule = Rule::new(
                        nf::types::NfFamily::INet,
                        "netgraph".into(),
                        chain_name.clone().to_string(),
                        current_subpath
                            .iter()
                            .map(|rule| nf::stmt::Statement::Match(rule.clone()))
                            .chain(vec![nf::stmt::Statement::SNAT(Some(nf::stmt::NAT {
                                addr: Some(Expression::String(addr.clone())),
                                family: Some(NATFamily::IP),
                                port: Some(
                                    addr.clone()
                                        .split(':')
                                        .last()
                                        .unwrap()
                                        .to_string()
                                        .parse()?,
                                ),
                                flags: None,
                            }))])
                            .collect(),
                    );
                    objects.push(CmdObject(NfCmd::Add(NfListObject::Chain(chain))));
                    objects.push(CmdObject(NfCmd::Add(NfListObject::Rule(rule))));
                    chain_name += 1;
                    current_subpath.clear();
                    current_subpath.push(Match {
                        left: Expression::String("saddr".into()),
                        right: Expression::String(addr.to_string()),
                        op: Operator::EQ,
                    });
                }
                "core:destination_nat" => {
                    let hook = if is_incoming {
                        nf::types::NfHook::Input
                    } else {
                        nf::types::NfHook::Output
                    };
                    let addr = predicate
                        .params
                        .get("addr")
                        .ok_or(anyhow::anyhow!("Address is required"))?;
                    let chain = Chain::new(
                        nf::types::NfFamily::INet,
                        "netgraph".into(),
                        chain_name.clone().to_string(),
                        Some(nf::types::NfChainType::NAT),
                        Some(hook),
                        None,
                        None,
                        Some(nf::types::NfChainPolicy::Accept),
                    );
                    let rule = Rule::new(
                        nf::types::NfFamily::INet,
                        "netgraph".into(),
                        chain_name.clone().to_string(),
                        current_subpath
                            .iter()
                            .map(|rule| nf::stmt::Statement::Match(rule.clone()))
                            .chain(vec![nf::stmt::Statement::DNAT(Some(nf::stmt::NAT {
                                addr: Some(Expression::String(addr.clone())),
                                family: Some(NATFamily::IP),
                                port: Some(
                                    addr.clone()
                                        .split(':')
                                        .last()
                                        .unwrap()
                                        .to_string()
                                        .parse()?,
                                ),
                                flags: None,
                            }))])
                            .collect(),
                    );
                    objects.push(CmdObject(NfCmd::Add(NfListObject::Chain(chain))));
                    objects.push(CmdObject(NfCmd::Add(NfListObject::Rule(rule))));
                    chain_name += 1;
                    current_subpath.clear();
                    current_subpath.push(Match {
                        left: Expression::String("daddr".into()),
                        right: Expression::String(addr.to_string()),
                        op: Operator::EQ,
                    });
                }
                "core:source" => {
                    is_incoming = true;
                }
                "core:localhost" => {
                    let chain = Chain::new(
                        nf::types::NfFamily::INet,
                        "netgraph".into(),
                        chain_name.clone().to_string(),
                        Some(nf::types::NfChainType::Filter),
                        Some(nf::types::NfHook::Input),
                        None,
                        None,
                        Some(nf::types::NfChainPolicy::Drop),
                    );
                    let rule = Rule::new(
                        nf::types::NfFamily::INet,
                        "netgraph".into(),
                        chain_name.clone().to_string(),
                        current_subpath
                            .iter()
                            .map(|rule| nf::stmt::Statement::Match(rule.clone()))
                            .chain(vec![nf::stmt::Statement::Accept(Some(nf::stmt::Accept {}))])
                            .collect(),
                    );
                    objects.push(CmdObject(NfCmd::Add(NfListObject::Chain(chain))));
                    objects.push(CmdObject(NfCmd::Add(NfListObject::Rule(rule))));
                    chain_name += 1;
                    current_subpath.clear();
                    is_incoming = false;
                }
                "core:drop" => {
                    let hook = if is_incoming {
                        nf::types::NfHook::Input
                    } else {
                        nf::types::NfHook::Output
                    };
                    let chain = Chain::new(
                        nf::types::NfFamily::INet,
                        "netgraph".into(),
                        chain_name.clone().to_string(),
                        Some(nf::types::NfChainType::Filter),
                        Some(hook),
                        None,
                        None,
                        Some(nf::types::NfChainPolicy::Accept),
                    );
                    let rule = Rule::new(
                        nf::types::NfFamily::INet,
                        "netgraph".into(),
                        chain_name.clone().to_string(),
                        current_subpath
                            .iter()
                            .map(|rule| nf::stmt::Statement::Match(rule.clone()))
                            .chain(vec![nf::stmt::Statement::Drop(Some(nf::stmt::Drop {}))])
                            .collect(),
                    );
                    objects.push(CmdObject(NfCmd::Add(NfListObject::Chain(chain))));
                    objects.push(CmdObject(NfCmd::Add(NfListObject::Rule(rule))));
                }
                "core:accept" => {
                    let chain = Chain::new(
                        nf::types::NfFamily::INet,
                        "netgraph".into(),
                        chain_name.clone().to_string(),
                        Some(nf::types::NfChainType::Filter),
                        Some(nf::types::NfHook::Output),
                        None,
                        None,
                        Some(nf::types::NfChainPolicy::Accept),
                    );
                    let rule = Rule::new(
                        nf::types::NfFamily::INet,
                        "netgraph".into(),
                        chain_name.clone().to_string(),
                        current_subpath
                            .iter()
                            .map(|rule| nf::stmt::Statement::Match(rule.clone()))
                            .chain(vec![nf::stmt::Statement::Accept(Some(nf::stmt::Accept {}))])
                            .collect(),
                    );
                    objects.push(CmdObject(NfCmd::Add(NfListObject::Chain(chain))));
                    objects.push(CmdObject(NfCmd::Add(NfListObject::Rule(rule))));
                }
                _ => {
                    return Err(anyhow::anyhow!("Unknown node type: {}", variant));
                }
            };
        }
        Ok(objects)
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
                let node_id = stack.last()?.1.get(stack.last()?.0 - 1)?;
                stack.push((0, self.get_connected_receiver_nodes(*node_id)));
            } else {
                stack.pop();
            }
        }
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
                .insert(node, Pos2::default());
            self.editor_state.node_order.push(node);
        }
    }

    fn save_node_graph(&self) -> anyhow::Result<()> {
        use serde_json::to_value;
        let path = if let Some(path) = &self.current_graph_path {
            path.clone()
        } else if let Some(file) = rfd::FileDialog::new().pick_folder() {
            if file.read_dir()?.next().is_some() {
                return Err(anyhow::anyhow!("Directory is not empty"));
            }
            file
        } else {
            return Ok(());
        };
        let source_node = to_value(self.source_node)
            .or(Err(anyhow::anyhow!("source node is not serializable")))?;
        let editor_state = to_value(&self.editor_state)
            .or(Err(anyhow::anyhow!("editor state is not serializable")))?;
        let plugins = to_value(&self.user_state.plugins)
            .or(Err(anyhow::anyhow!("plugins is not serializable")))?;

        let mut map = Map::new();
        map.insert("source_node".to_string(), source_node);
        map.insert("editor_state".to_string(), editor_state);
        map.insert("plugins".to_string(), plugins);
        let json = serde_json::to_string(&map).unwrap();

        let graph_path = path
            .clone()
            .tap_mut(|s| s.as_mut_os_string().push("/graph.json"));
        let plugins_path = path
            .clone()
            .tap_mut(|s| s.as_mut_os_string().push("/plugins"));
        std::fs::write(graph_path, json).or(Err(anyhow::anyhow!("Cannot write graph file")))?;
        std::fs::create_dir_all(plugins_path)?;
        Ok(())
    }

    fn load_node_graph(&mut self) -> anyhow::Result<()> {
        let Some(path) = rfd::FileDialog::new().pick_folder() else {
            return Ok(());
        };
        let graph_path = path
            .clone()
            .tap_mut(|s| s.as_mut_os_string().push("/graph.json"));
        let json = std::fs::read_to_string(graph_path)
            .or(Err(anyhow::anyhow!("Cannot read graph file")))?;
        let map: Map<_, _> = serde_json::from_str(&json).or(Err(anyhow::anyhow!("")))?;
        let source_node = serde_json::from_value(
            map.get("source_node")
                .cloned()
                .ok_or(anyhow::anyhow!("incorrect file format"))?,
        )?;
        let editor_state = serde_json::from_value(
            map.get("editor_state")
                .cloned()
                .ok_or(anyhow::anyhow!("incorrect file format"))?,
        )?;
        let user_state_plugins = serde_json::from_value(
            map.get("plugins")
                .cloned()
                .ok_or(anyhow::anyhow!("incorrect file format"))?,
        )?;
        self.new_graph();
        self.source_node = source_node;
        self.user_state.plugins = user_state_plugins;
        self.editor_state = editor_state;
        self.current_graph_path = Some(path);
        self.reload_all_kinds();
        Ok(())
    }

    fn import_extension(&mut self) -> anyhow::Result<()> {
        // is there somewhere to import into
        let graph_storage = &self
            .current_graph_path
            .clone()
            .ok_or(anyhow::Error::msg("Save this graph first!"))?;
        let Some(plugin_source_dir) = rfd::FileDialog::new().pick_folder() else {
            return Ok(());
        };

        // read plugin manifest
        let plugin = plugin_source_dir
            .clone()
            .tap_mut(|s| s.as_mut_os_string().push("/plugin.json"));
        let plugin = std::fs::read_to_string(plugin)?;
        let plugin: Plugin = serde_json::from_str(&plugin)
            .or(Err(anyhow::anyhow!("Incorrect plugin.json format")))?;

        let plugin_dest_dir = graph_storage
            .clone()
            .tap_mut(|s| s.as_mut_os_string().push(format!("/plugins/{}", plugin.id)));
        let plugin_source_script = plugin_source_dir
            .clone()
            .tap_mut(|s| s.as_mut_os_string().push(format!("/{}", plugin.id)));
        let plugin_dest_script = plugin_dest_dir
            .clone()
            .tap_mut(|s| s.as_mut_os_string().push(format!("/{}", plugin.id)));

        if !plugin_source_script.exists() {
            return Err(anyhow::anyhow!("Plugin script not found"));
        }
        let mut node_scripts = vec![];
        for (node_id, _) in &plugin.nf {
            let node_script = plugin_source_dir
                .clone()
                .tap_mut(|s| s.as_mut_os_string().push(format!("/{node_id}")));
            if !node_script.exists() {
                return Err(anyhow::anyhow!(format!(
                    "Script for node {node_id} not found",
                )));
            }
            node_scripts.push(node_script);
        }

        if let Err(e) = std::fs::create_dir_all(&plugin_dest_dir) {
            if e.kind() != std::io::ErrorKind::AlreadyExists {
                return Err(anyhow::anyhow!("Cannot create plugin directory: {}", e));
            }
        }
        if let Err(e) = std::fs::copy(plugin_source_script, plugin_dest_script) {
            return Err(anyhow::anyhow!("Cannot copy plugin script: {}", e));
        }
        for node_script in node_scripts {
            let node_script_dest = plugin_dest_dir
                .clone()
                .tap_mut(|s| s.as_mut_os_string().push("/"))
                .tap_mut(|s| s.as_mut_os_string().push(node_script.file_name().unwrap()));
            if let Err(e) = std::fs::copy(&node_script, &node_script_dest) {
                return Err(anyhow::anyhow!(
                    "Cannot copy node script {}: {e}",
                    node_script.to_string_lossy()
                ));
            }
        }

        let mut plugin_nodes = hash_map! {};
        for (node_id, node) in &plugin.nf {
            plugin_nodes.insert(node_id.clone(), node.clone());
        }
        self.user_state
            .plugins
            .insert(plugin.id.clone(), plugin_nodes);

        self.reload_all_kinds();
        Ok(())
    }

    fn export_configuration(&self) -> anyhow::Result<()> {
        let Some(save_path) = rfd::FileDialog::new().pick_folder() else {
            return Ok(());
        };
        if save_path.read_dir()?.next().is_some() {
            return Err(anyhow::anyhow!("Directory is not empty"));
        }
        let nft_json_path = save_path
            .clone()
            .tap_mut(|s| s.as_mut_os_string().push("/nft.json"));

        let mut node_output_db = NodeOutputDB::new();
        for node_id in self.editor_state.graph.iter_nodes() {
            self.recurse_node_outputs(node_id, &mut node_output_db)?;
        }
        let nf_objects: Vec<NfObject> = self
            .editor_state
            .graph
            .nodes
            .iter()
            .filter(|(_, node)| node.outputs.is_empty())
            .filter_map(|(node_id, _)| Some(node_output_db.get(&node_id)?.get("terminal")?.clone()))
            .flatten()
            .filter_map(|path| Self::evaluate_path(&path).ok())
            .flatten()
            .collect();
        let table = NfObject::CmdObject(NfCmd::Add(NfListObject::Table(
            nftables::schema::Table::new(nftables::types::NfFamily::INet, "netgraph".into()),
        )));
        let nft = Nftables {
            objects: [vec![table], nf_objects].concat(),
        };
        let nft = serde_json::to_string_pretty(&nft)
            .ok()
            .ok_or(anyhow::anyhow!("rules serialization failed"))?;
        std::fs::write(nft_json_path, nft)?;
        Ok(())
    }
}

impl Hash for Predicate {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.variant.hash(state);
        for (key, value) in &self.params {
            key.hash(state);
            value.hash(state);
        }
    }
}
