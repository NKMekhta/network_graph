use std::collections::HashMap;
use std::fmt::Display;

use serde::{Deserialize, Serialize};

use super::nf_graph::{NFDirection, NFFamily};

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct NFInput {
    pub family: NFFamily,
    pub direction: NFDirection,
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct NFOutput {
    pub family: NFFamily,
    pub direction: NFDirection,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct NFNode {
    pub display_name: String,
    pub params: HashMap<String, String>,
    pub input: NFInput,
    pub outputs: HashMap<String, NFOutput>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Plugin {
    pub id: String,
    pub nf: HashMap<String, NFNode>,
}

impl Display for NFNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name)
    }
}

fn main() {
    // let a = Plugin {
    //     id: "jool".to_string(),
    //     nf: Some(NFTables {
    //         namespaces: vec!["jool".to_string()],
    //         nodes: vec![
    //             NFNode {
    //                 id: "siit".to_string(),
    //                 options: vec![NFOption {
    //                     id: "prefix".to_string(),
    //                     name: "Prefix".to_string(),
    //                     variadic: false,
    //                 }],
    //                 inputs: vec![
    //                     NFInput {
    //                         id: "v4in".to_string(),
    //                         option_id: "prefix".to_string(),
    //                         each_option: false,
    //                         family: NfFamily::Inet,
    //                         direction: NFDirection::Either,
    //                     },
    //                     NFInput {
    //                         id: "v6in".to_string(),
    //                         option_id: "prefix".to_string(),
    //                         each_option: false,
    //                         family: NfFamily::Inet,
    //                         direction: NFDirection::Either,
    //                     }
    //                 ],
    //                 outputs: vec![
    //                     NFOutput {
    //                         id: "v4out".to_string(),
    //                         option_id: "prefix".to_string(),
    //                         each_option: false,
    //                         family: NfFamily::Inet,
    //                         direction: NFDirection::Either,
    //                     },
    //                     NFOutput {
    //                         id: "siit".to_string(),
    //                         option_id: "prefix".to_string(),
    //                         each_option: false,
    //                         family: NfFamily::Inet,
    //                         direction: NFDirection::Either,
    //                     },
    //                 ],
    //             }
    //         ],
    //     }),
    // };
}
