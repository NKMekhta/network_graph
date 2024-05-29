use std::borrow::Cow;

use derive_more::Constructor;
use eframe::egui::Color32;
use serde::{Deserialize, Serialize};

use egui_node_graph::DataTypeTrait;

#[derive(Default, Debug, Copy, Clone, Serialize, Deserialize, PartialEq)]
pub enum NFFamily {
    #[default]
    Inet,
    IPv4,
    IPv6,
}

#[derive(Default, Debug, Copy, Clone, Serialize, Deserialize, PartialEq)]
pub enum NFDirection {
    #[default]
    Either,
    Incoming,
    Outgoing,
}

#[derive(Constructor)]
pub struct DataType {
    family: NFFamily,
    pub(crate) direction: NFDirection,
}

impl DataTypeTrait<super::NFGraphState> for DataType {
    fn data_type_color(&self, _user_state: &mut super::NFGraphState) -> Color32 {
        match self.direction {
            NFDirection::Either => Color32::LIGHT_GRAY,
            NFDirection::Incoming => Color32::LIGHT_RED,
            NFDirection::Outgoing => Color32::LIGHT_BLUE,
        }
    }

    fn name(&self) -> Cow<str> {
        let family = match self.family {
            NFFamily::Inet => "inet",
            NFFamily::IPv4 => "ipv4",
            NFFamily::IPv6 => "ipv6",
        };
        let direction = match self.direction {
            NFDirection::Either => "",
            NFDirection::Incoming => "Incoming ",
            NFDirection::Outgoing => "Outgoing ",
        };
        Cow::from(direction.to_owned() + family)
    }
}

impl PartialEq for DataType {
    fn eq(&self, other: &Self) -> bool {
        (self.direction == NFDirection::Either
            || other.direction == NFDirection::Either
            || self.direction == other.direction)
            && (self.family == NFFamily::Inet
                || other.family == NFFamily::Inet
                || self.family == other.family)
    }
}

impl Eq for DataType {}
