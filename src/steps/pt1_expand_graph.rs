use std::collections::BTreeMap;
use std::path::Path;

use crate::steps::pt2_resolve_nodes::find_definition;
use crate::structs::mmg::{NodeLibrary, ParameterValue};
use crate::structs::ptex::{Connection, Node, Project};

pub fn expand_graphs(project: &Project, library: &NodeLibrary) -> Result<Project, String> {
    let flat = flatten_graph(project.nodes.clone(), project.connections.clone(), "", &project.path)?;
    let mut expanded = project.clone();
    expanded.nodes = flat.nodes;
    expanded.connections = flat.connections;
    loop {
        let mut graph_index = None;
        for (index, node) in expanded.nodes.iter().enumerate() {
            let Some(definition) = find_definition(library, &node.r#type) else { continue };
            if definition.r#type == "graph" {
                graph_index = Some(index);
                break;
            }
        }
        let Some(index) = graph_index else { break };
        let wrapper = expanded.nodes[index].clone();
        let mut definition = find_definition(library, &wrapper.r#type).expect("graph definition was found").clone();
        apply_remote_parameters(&mut definition, &wrapper.parameters)?;
        let flat = flatten_graph(definition.nodes, definition.connections, &wrapper.name, &expanded.path)?;
        let mut rewritten = Vec::new();
        for connection in &expanded.connections {
            let mut connection = connection.clone();
            if connection.from == wrapper.name {
                let source = resolve_graph_output(&flat, &wrapper, connection.from_port, &expanded.connections)?;
                connection.from = source.node;
                connection.from_port = source.port;
            }
            if connection.to != wrapper.name {
                rewritten.push(connection);
                continue;
            }
            let Some(destinations) = flat.inputs.get(&connection.to_port) else { continue };
            for destination in destinations {
                let mut input_connection = connection.clone();
                input_connection.to = destination.node.clone();
                input_connection.to_port = destination.port;
                rewritten.push(input_connection);
            }
        }
        rewritten.extend(flat.connections);
        expanded.connections = rewritten;
        expanded.nodes.remove(index);
        expanded.nodes.extend(flat.nodes);
    }
    Ok(expanded)
}

#[derive(Clone)]
struct Endpoint {
    node: String,
    port: usize,
}

enum GraphOutput {
    Node(Endpoint),
    Input(usize),
}

#[derive(Default)]
struct FlatGraph {
    nodes: Vec<Node>,
    connections: Vec<Connection>,
    inputs: BTreeMap<usize, Vec<Endpoint>>,
    outputs: BTreeMap<usize, GraphOutput>,
}

fn flatten_graph(
    nodes: Vec<Node>,
    connections: Vec<Connection>,
    prefix: &str,
    path: &Path,
) -> Result<FlatGraph, String> {
    let mut graph = FlatGraph::default();
    let mut nested = BTreeMap::<String, FlatGraph>::new();
    let mut names = BTreeMap::<String, String>::new();
    for mut node in nodes {
        if node.r#type == "remote" {
            continue;
        }
        if node.r#type == "graph" {
            let child_prefix = scoped_name(prefix, &node.name);
            nested.insert(node.name, flatten_graph(node.nodes, node.connections, &child_prefix, path)?);
            continue;
        }
        if node.r#type == "ios" && (node.name == "gen_inputs" || node.name == "gen_outputs") {
            continue;
        }
        let scoped = scoped_name(prefix, &node.name);
        names.insert(node.name, scoped.clone());
        node.name = scoped;
        graph.nodes.push(node);
    }
    for connection in connections {
        let Connection { from, from_port, to, to_port } = connection;
        if from == "gen_inputs" && to == "gen_outputs" {
            graph.outputs.insert(to_port, GraphOutput::Input(from_port));
        } else if from == "gen_inputs" {
            for destination in resolve_dest(&to, to_port, &nested, &names, path, prefix)? {
                graph.inputs.entry(from_port).or_default().push(destination);
            }
        } else if to == "gen_outputs" {
            let sources = resolve_source(&from, from_port, &nested, &names, path, prefix)?;
            if let Some(source) = sources.into_iter().next() {
                graph.outputs.insert(to_port, GraphOutput::Node(source));
            }
        } else {
            let sources = resolve_source(&from, from_port, &nested, &names, path, prefix)?;
            let destinations = resolve_dest(&to, to_port, &nested, &names, path, prefix)?;
            for source in &sources {
                for destination in &destinations {
                    graph.connections.push(Connection {
                        from: source.node.clone(),
                        from_port: source.port,
                        to: destination.node.clone(),
                        to_port: destination.port,
                    });
                }
            }
        }
    }
    for (_, child) in nested {
        graph.connections.extend(child.connections);
        graph.nodes.extend(child.nodes);
    }
    Ok(graph)
}

fn resolve_source(
    name: &str,
    port: usize,
    nested: &BTreeMap<String, FlatGraph>,
    names: &BTreeMap<String, String>,
    path: &Path,
    prefix: &str,
) -> Result<Vec<Endpoint>, String> {
    if let Some(child) = nested.get(name) {
        let output = child
            .outputs
            .get(&port)
            .ok_or_else(|| format!("{}: nested graph `{name}` has no connected output {port}", path.display()))?;
        return match output {
            GraphOutput::Node(endpoint) => Ok(vec![endpoint.clone()]),
            GraphOutput::Input(_) => Err(format!(
                "{}: nested graph `{name}` output {port} forwards an input through a parent graph",
                path.display()
            )),
        };
    }
    let node = names
        .get(name)
        .ok_or_else(|| format!("{}: node `{name}` does not exist in graph `{prefix}`", path.display()))?;
    Ok(vec![Endpoint { node: node.clone(), port }])
}

fn resolve_dest(
    name: &str,
    port: usize,
    nested: &BTreeMap<String, FlatGraph>,
    names: &BTreeMap<String, String>,
    path: &Path,
    prefix: &str,
) -> Result<Vec<Endpoint>, String> {
    if let Some(child) = nested.get(name) {
        return child
            .inputs
            .get(&port)
            .cloned()
            .ok_or_else(|| format!("{}: nested graph `{name}` has no connected input {port}", path.display()));
    }
    let node = names
        .get(name)
        .ok_or_else(|| format!("{}: node `{name}` does not exist in graph `{prefix}`", path.display()))?;
    Ok(vec![Endpoint { node: node.clone(), port }])
}

fn resolve_graph_output(
    graph: &FlatGraph,
    wrapper: &Node,
    output_port: usize,
    connections: &[Connection],
) -> Result<Endpoint, String> {
    match graph
        .outputs
        .get(&output_port)
        .ok_or_else(|| format!("graph node `{}` has no connected output {output_port}", wrapper.name))?
    {
        GraphOutput::Node(endpoint) => Ok(endpoint.clone()),
        GraphOutput::Input(input_port) => {
            let connection = connections
                .iter()
                .find(|connection| connection.to == wrapper.name && connection.to_port == *input_port)
                .ok_or_else(|| {
                    format!(
                        "graph node `{}` output {output_port} forwards unconnected input {input_port}",
                        wrapper.name
                    )
                })?;
            Ok(Endpoint { node: connection.from.clone(), port: connection.from_port })
        }
    }
}

fn apply_remote_parameters(
    graph: &mut Node,
    instance_parameters: &BTreeMap<String, ParameterValue>,
) -> Result<(), String> {
    let mut remotes = Vec::new();
    for node in &graph.nodes {
        if node.r#type == "remote" {
            remotes.push(node.clone());
        }
    }
    for remote in remotes {
        for widget in &remote.widgets {
            let mut value = instance_parameters.get(&widget.name);
            if value.is_none() {
                value = remote.parameters.get(&widget.name);
            }
            if value.is_none() {
                value = graph.parameters.get(&widget.name);
            }
            let Some(value) = value.cloned() else { continue };
            match widget.r#type.as_str() {
                "linked_control" => {
                    for link in &widget.linked_widgets {
                        let target =
                            graph.nodes.iter_mut().find(|node| node.name == link.node).ok_or_else(|| {
                                format!("graph remote parameter refers to missing node `{}`", link.node)
                            })?;
                        target.parameters.insert(link.widget.clone(), value.clone());
                    }
                }
                "config_control" => {
                    let selected = match value {
                        ParameterValue::Boolean(value) => usize::from(value),
                        ParameterValue::Number(value) if value >= 0.0 => value as usize,
                        _ => 0,
                    };
                    if let Some(actions) = widget.configurations.values().nth(selected) {
                        for action in actions {
                            let target =
                                graph.nodes.iter_mut().find(|node| node.name == action.node).ok_or_else(|| {
                                    format!("graph remote parameter refers to missing node `{}`", action.node)
                                })?;
                            target.parameters.insert(action.widget.clone(), action.value.clone());
                        }
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn scoped_name(prefix: &str, name: &str) -> String {
    if prefix.is_empty() { name.to_owned() } else { format!("{prefix}__{name}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expansion_applies_controls_and_reconnects_graph_ports() {
        let project: Project = serde_json::from_str(
            r#"{
            "nodes": [
                {"name": "source", "type": "shader"},
                {"name": "wrapped", "type": "operation", "parameters": {"amount": 7}},
                {"name": "target", "type": "material"}
            ],
            "connections": [
                {"from": "source", "from_port": 2, "to": "wrapped", "to_port": 0},
                {"from": "wrapped", "from_port": 0, "to": "target", "to_port": 1}
            ]
        }"#,
        )
        .unwrap();
        let definition: Node = serde_json::from_str(
            r#"{
            "name": "operation", "type": "graph",
            "nodes": [
                {"name": "gen_inputs", "type": "ios"},
                {"name": "op", "type": "shader", "parameters": {"amount": 1}},
                {"name": "gen_outputs", "type": "ios"},
                {"name": "controls", "type": "remote", "widgets": [
                    {"name": "amount", "type": "linked_control", "linked_widgets": [
                        {"node": "op", "widget": "amount"}
                    ]}
                ]}
            ],
            "connections": [
                {"from": "gen_inputs", "from_port": 0, "to": "op", "to_port": 0},
                {"from": "op", "from_port": 1, "to": "gen_outputs", "to_port": 0}
            ]
        }"#,
        )
        .unwrap();
        let mut library = NodeLibrary::default();
        library.definitions.insert(definition.name.clone(), definition);
        let expanded = expand_graphs(&project, &library).unwrap();

        let operation = expanded.nodes.iter().find(|node| node.name == "wrapped__op").unwrap();
        assert!(matches!(operation.parameters["amount"], ParameterValue::Number(7.0)));
        assert_eq!(expanded.connections.len(), 2);
        assert!(
            expanded
                .connections
                .iter()
                .any(|c| c.from == "source" && c.from_port == 2 && c.to == "wrapped__op" && c.to_port == 0)
        );
        assert!(
            expanded
                .connections
                .iter()
                .any(|c| c.from == "wrapped__op" && c.from_port == 1 && c.to == "target" && c.to_port == 1)
        );
        assert_eq!(project.nodes[1].name, "wrapped");
    }
}
