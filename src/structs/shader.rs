use std::collections::BTreeMap;

use crate::structs::ptex::Node;

#[derive(Debug)]
pub struct ShaderPlan {
    pub nodes: Vec<PlannedNode>,
    pub target: ShaderOutput,
    pub output_type: String,
    pub globals: Vec<String>,
}

#[derive(Debug)]
pub struct PlannedNode {
    pub output: ShaderOutput,
    pub node_index: usize,
    pub output_type: String,
    pub node: Node,
    pub inputs: BTreeMap<usize, PlannedInput>,
}

#[derive(Debug)]
pub struct PlannedInput {
    pub output: ShaderOutput,
    pub output_type: String,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ShaderOutput {
    pub node: String,
    pub port: usize,
}

pub struct ShaderFragment {
    pub output: ShaderOutput,
    pub global: String,
    pub instance: String,
    pub function: String,
}

pub(crate) struct SourceBlock {
    pub signature: Option<String>,
    pub text: String,
}
