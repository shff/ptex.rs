use std::collections::{BTreeMap, BTreeSet};

use crate::shader_templates::{
    expand_generic_code, generic_input_index, matching_paren, output_expression, output_name, parameter_number,
    source_text, switch_input,
};
use crate::steps::pt2_resolve_nodes::find_definition;
use crate::structs::mmg::{NodeLibrary, ParameterValue};
use crate::structs::ptex::{Node, Project};
use crate::structs::shader::{PlannedInput, PlannedNode, ShaderOutput, ShaderPlan};

pub fn plan_shader(
    project: &Project,
    library: &NodeLibrary,
    target: &Node,
    output: usize,
) -> Result<ShaderPlan, String> {
    let target = ShaderOutput { node: target.name.clone(), port: output };
    let mut pending = vec![(target.clone(), false)];
    let mut active = BTreeSet::new();
    let mut completed = BTreeMap::<ShaderOutput, String>::new();
    let mut nodes = Vec::new();
    let mut includes = Vec::new();
    let mut function_names = BTreeMap::new();

    // Visit each required output after its dependencies have been completed.
    while let Some((output, dependencies_visited)) = pending.pop() {
        if completed.contains_key(&output) {
            continue;
        }
        let node_index = project.nodes.iter().position(|node| node.name == output.node);
        let Some(node_index) = node_index else {
            return Err(format!("connection source `{}` does not exist", output.node));
        };
        let node = &project.nodes[node_index];
        let input_indices = referenced_inputs(project, node, output.port)?;
        let mut inputs = BTreeMap::new();
        for index in input_indices {
            let connection = project.connections.iter().find(|c| c.to == node.name && c.to_port == index);
            if let Some(connection) = connection {
                let source = ShaderOutput { node: connection.from.clone(), port: connection.from_port };
                inputs.insert(index, source);
            }
        }

        if !dependencies_visited {
            if !active.insert(output.clone()) {
                return Err(format!("graph cycle reaches node `{}` output {}", output.node, output.port));
            }
            pending.push((output.clone(), true));
            for source in inputs.values().rev() {
                pending.push((source.clone(), false));
            }
            continue;
        }

        let mut planned_inputs = BTreeMap::new();
        for (index, source) in inputs {
            let output_type = completed.get(&source).ok_or("shader dependency was not planned")?;
            planned_inputs.insert(index, PlannedInput { output: source, output_type: output_type.clone() });
        }
        let output_type;
        if node.r#type == "switch" || node.r#type == "buffer" {
            // These nodes preserve the type of the input they evaluate.
            let selected = if node.r#type == "switch" { switch_input(node, output.port) } else { 0 };
            output_type = match planned_inputs.get(&selected) {
                Some(input) => input.output_type.clone(),
                None => "rgba".to_owned(),
            };
        } else {
            let model = node.shader_model.as_ref().ok_or("node has no shader model")?;
            let model_output = model.outputs.get(output.port).ok_or("node has no requested output")?;
            output_type = model_output.r#type.clone();
            includes.extend(model.includes.clone());
        }
        active.remove(&output);
        let function_name = output_name(&output);
        if let Some(previous) = function_names.insert(function_name.clone(), output.clone()) {
            if previous != output {
                return Err(format!(
                    "nodes `{}` and `{}` produce the same shader function `{function_name}`",
                    previous.node, output.node
                ));
            }
        }
        completed.insert(output.clone(), output_type.clone());
        nodes.push(PlannedNode { output, node: node.clone(), node_index, output_type, inputs: planned_inputs });
    }

    let output_type = completed.get(&target).ok_or("shader target was not planned")?.clone();
    let mut globals = vec![library.common_globals.clone()];
    globals.extend(include_sources(library, &includes)?);
    Ok(ShaderPlan { nodes, target, output_type, globals })
}

fn referenced_inputs(project: &Project, node: &Node, output: usize) -> Result<BTreeSet<usize>, String> {
    let mut inputs = BTreeSet::new();
    if node.r#type == "switch" {
        inputs.insert(switch_input(node, output));
        return Ok(inputs);
    }
    if node.r#type == "buffer" {
        inputs.insert(0);
        return Ok(inputs);
    }
    let model = node.shader_model.as_ref().ok_or_else(|| format!("node `{}` has no shader model", node.name))?;
    let model_output =
        model.outputs.get(output).ok_or_else(|| format!("node `{}` has no output {output}", node.name))?;
    let expression = output_expression(model_output)
        .ok_or_else(|| format!("node `{}` output {output} has no expression", node.name))?;
    let generic_size = node.generic_size.unwrap_or(1).max(1);
    let mut sources = vec![
        source_text(&model.global),
        expand_generic_code(&source_text(&model.instance), generic_size),
        expand_generic_code(&source_text(&model.code), generic_size),
        expression.replace('#', &(output + 1).to_string()),
    ];
    let mut inspected = BTreeSet::new();

    // Inspect nested arguments, selected enum expressions, and unconnected-input defaults.
    // Only referenced inputs become dependencies; unused connections are left out.
    while let Some(source) = sources.pop() {
        if !inspected.insert(source.clone()) {
            continue;
        }
        let mut cursor = 0;
        while let Some(offset) = source[cursor..].find('$') {
            let start = cursor + offset + 1;
            let token;
            let end;
            if source.as_bytes().get(start) == Some(&b'(') {
                end = matching_paren(&source, start).ok_or("unterminated shader variable")? + 1;
                token = &source[start + 1..end - 1];
            } else {
                let mut token_end = start;
                while token_end < source.len() {
                    let byte = source.as_bytes()[token_end];
                    if !byte.is_ascii_alphanumeric() && byte != b'_' && byte != b'?' && byte != b'#' {
                        break;
                    }
                    token_end += 1;
                }
                token = &source[start..token_end];
                end = token_end;
            }
            cursor = end;

            let mut input = generic_input_index(model, token, generic_size);
            if token == "input" {
                input = Some((0, 0));
            } else if let Some(index) = token.strip_prefix("input_") {
                let index = index.parse::<usize>().map_err(|e| e.to_string())?;
                input = Some((index, index));
            }
            if let Some((index, definition_index)) = input {
                if inputs.insert(index) {
                    let input = model.inputs.get(definition_index).ok_or("shader input does not exist")?;
                    let connected = project.connections.iter().any(|c| c.to == node.name && c.to_port == index);
                    if !connected {
                        if let Some(default) = &input.default {
                            sources.push(default.clone());
                        }
                    }
                }
                continue;
            }

            let parameter_name = token.strip_prefix("param:").unwrap_or(token);
            let value = node.parameters.get(parameter_name);
            let definition = model
                .parameters
                .iter()
                .find(|parameter| parameter.name.replace('#', "1") == parameter_name.replace('#', "1"));
            if let Some(definition) = definition {
                if definition.r#type == "enum" {
                    let selected = value.and_then(parameter_number).unwrap_or(0.0).max(0.0) as usize;
                    let mut selected_value = definition.values.get(selected);
                    if selected_value.is_none() {
                        selected_value = definition.values.first();
                    }
                    if let Some(selected_value) = selected_value {
                        sources.push(selected_value.value.clone());
                    }
                }
            }
            if let Some(ParameterValue::String(expression)) = value {
                sources.push(expression.clone());
            }
        }
    }
    Ok(inputs)
}

fn include_sources(library: &NodeLibrary, roots: &[String]) -> Result<Vec<String>, String> {
    let mut pending = Vec::new();
    for name in roots.iter().rev() {
        pending.push((name.clone(), false));
    }
    let mut active = BTreeSet::new();
    let mut completed = BTreeSet::new();
    let mut sources = Vec::new();
    while let Some((name, dependencies_visited)) = pending.pop() {
        if completed.contains(&name) {
            continue;
        }
        let definition = find_definition(library, &name).ok_or_else(|| format!("shader include `{name}` not found"))?;
        let model =
            definition.shader_model.as_ref().ok_or_else(|| format!("shader include `{name}` has no shader model"))?;
        if !dependencies_visited {
            if !active.insert(name.clone()) {
                return Err(format!("shader include cycle reaches `{name}`"));
            }
            pending.push((name.clone(), true));
            for include in model.includes.iter().rev() {
                pending.push((include.clone(), false));
            }
            continue;
        }
        active.remove(&name);
        completed.insert(name);
        sources.push(source_text(&model.global));
    }
    Ok(sources)
}
