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

#[cfg(test)]
mod tests {
    use map_macro::hash_map;

    use super::*;

    #[test]
    fn test_nf_node() {
        let a = Plugin {
            id: "test_plugin".to_string(),
            nf: hash_map! {
                "test".to_string() => NFNode {
                    display_name: "test_node".to_string(),
                    params: hash_map! {
                        "param1".to_string() => "Parameter A".to_string(),
                        "param2".to_string() => "Parameter B".to_string(),
                    },
                    input: NFInput {
                        family: NFFamily::Inet,
                        direction: NFDirection::Either,
                    },
                    outputs: hash_map! {
                        "output1".to_string() => NFOutput {
                            family: NFFamily::Inet,
                            direction: NFDirection::Either,
                        },
                        "output2".to_string() => NFOutput {
                            family: NFFamily::Inet,
                            direction: NFDirection::Either,
                        },
                    }
                }
            },
        };
        let a = serde_json::to_string_pretty(&a).unwrap();
        println!("{a}");
    }
}
