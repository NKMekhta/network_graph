use derive_more::Constructor;

#[derive(Constructor, Clone)]
pub struct NodeTemplateIter {
    additional_kinds: Vec<super::NFNodeData>,
}

impl egui_node_graph::NodeTemplateIter for NodeTemplateIter {
    type Item = super::NFNodeData;

    fn all_kinds(&self) -> Vec<Self::Item> {
        use super::NFNodeData::{
            Accept, DestinationAddressFilter, DestinationNAT, DestinationPortFilter, Drop,
            FamilySplitter, FileIpList, InterfaceFilter, ProtocolFilter, SourceAddressFilter,
            SourceNAT, SourcePortFilter,
        };
        let core_kinds = vec![
            InterfaceFilter(String::new()),
            FileIpList(None),
            SourceAddressFilter(String::new()),
            DestinationAddressFilter(String::new()),
            SourcePortFilter(String::new()),
            DestinationPortFilter(String::new()),
            ProtocolFilter(String::new()),
            FamilySplitter,
            SourceNAT(String::new()),
            DestinationNAT(String::new()),
            Drop,
            Accept,
        ];

        [core_kinds, self.additional_kinds.clone()].concat()
    }
}
