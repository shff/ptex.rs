//! Export profile condition evaluation.

use crate::structs::mmg::ShaderModel;
use crate::structs::ptex::{Node, Project};

pub fn profile_condition_matches(
    condition: &str,
    model: &ShaderModel,
    project: &Project,
    material: &Node,
) -> Result<bool, String> {
    if condition.trim().is_empty() {
        return Ok(true);
    }
    let mut any_term = false;
    for term in condition.split(" or ") {
        let mut all_connected = true;
        for atom in term.split(" and ") {
            let atom = atom.trim();
            let Some(input_name) = atom.strip_prefix("$(connected:") else {
                return Err(format!("unsupported export condition `{atom}`"));
            };
            let Some(input_name) = input_name.strip_suffix(')') else {
                return Err(format!("unsupported export condition `{atom}`"));
            };
            let mut input_index = None;
            for (index, input) in model.inputs.iter().enumerate() {
                if input.name == input_name {
                    input_index = Some(index);
                    break;
                }
            }
            let Some(input_index) = input_index else {
                return Err(format!("export condition refers to unknown input `{input_name}`"));
            };
            let mut connected = false;
            for connection in &project.connections {
                if connection.to == material.name && connection.to_port == input_index {
                    connected = true;
                    break;
                }
            }
            all_connected &= connected;
        }
        any_term |= all_connected;
    }
    Ok(any_term)
}
