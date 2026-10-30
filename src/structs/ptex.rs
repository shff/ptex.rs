use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Deserialize;

use crate::structs::mmg::{ParameterValue, ShaderModel};

#[derive(Clone, Debug, Deserialize)]
pub struct Project {
    #[serde(skip)]
    pub path: PathBuf,
    #[serde(default)]
    pub name: String,
    pub label: Option<String>,
    pub nodes: Vec<Node>,
    #[serde(default)]
    pub connections: Vec<Connection>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Node {
    #[serde(default)]
    pub name: String,
    pub r#type: String,
    pub seed: Option<f64>,
    pub seed_int: Option<f64>,
    pub generic_size: Option<usize>,
    #[serde(default)]
    pub parameters: BTreeMap<String, ParameterValue>,
    pub shader_model: Option<ShaderModel>,
    #[serde(default)]
    pub nodes: Vec<Node>,
    #[serde(default)]
    pub connections: Vec<Connection>,
    #[serde(default)]
    pub widgets: Vec<Widget>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Connection {
    pub from: String,
    pub from_port: usize,
    pub to: String,
    pub to_port: usize,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Widget {
    pub name: String,
    pub r#type: String,
    #[serde(default)]
    pub linked_widgets: Vec<WidgetLink>,
    #[serde(default)]
    pub configurations: BTreeMap<String, Vec<ConfigurationAction>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct WidgetLink {
    pub node: String,
    pub widget: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ConfigurationAction {
    pub node: String,
    pub widget: String,
    pub value: ParameterValue,
}
