#[derive(Default)]
pub struct ValueType;

impl egui_node_graph::WidgetValueTrait for ValueType {
    type Response = super::NodeResponse;

    type UserState = super::NFGraphState;

    type NodeData = super::NFNodeData;

    fn value_widget(
        &mut self,
        param_name: &str,
        node_id: egui_node_graph::NodeId,
        ui: &mut eframe::egui::Ui,
        user_state: &mut Self::UserState,
        node_data: &Self::NodeData,
    ) -> Vec<Self::Response> {
        self.value_widget_connected(param_name, node_id, ui, user_state, node_data)
    }
}
