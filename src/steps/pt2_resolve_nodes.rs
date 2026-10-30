use crate::structs::mmg::NodeLibrary;
use crate::structs::ptex::{Node, Project};

pub fn resolve_nodes(project: &Project, library: &NodeLibrary) -> Result<Project, String> {
    let mut resolved = project.clone();
    for node in &mut resolved.nodes {
        let definition = find_definition(library, &node.r#type);
        if let Some(definition) = definition {
            if node.shader_model.is_none() {
                node.shader_model = definition.shader_model.clone();
            }
            if node.generic_size.is_none() {
                node.generic_size = definition.generic_size;
            }
        }
        let Some(model) = &node.shader_model else { continue };
        for parameter in &model.parameters {
            let name = parameter.name.replace('#', "1");
            let mut value = node.parameters.get(&parameter.name);
            if value.is_none() {
                value = node.parameters.get(&name);
            }
            if value.is_none() {
                if let Some(base_name) = name.strip_suffix('1') {
                    value = node.parameters.get(base_name);
                }
            }
            if value.is_none() {
                value = parameter.default.as_ref();
            }
            let value = value.cloned();
            if let Some(value) = value {
                node.parameters.insert(parameter.name.clone(), value.clone());
                node.parameters.insert(name, value);
            }
        }
    }
    Ok(resolved)
}

pub fn find_definition<'a>(library: &'a NodeLibrary, name: &str) -> Option<&'a Node> {
    if let Some(definition) = library.definitions.get(name) {
        return Some(definition);
    }
    let alias = library.aliases.get(name)?;
    library.definitions.get(alias)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::structs::mmg::{ParameterObject, ParameterValue};

    #[test]
    fn resolution_preserves_instance_values_and_fills_missing_defaults() {
        let project: Project = serde_json::from_str(
            r#"{
            "nodes": [{"name": "noise", "type": "noise", "parameters": {"scale": 8}}]
        }"#,
        )
        .unwrap();
        let definition: Node = serde_json::from_str(
            r#"{
            "name": "noise", "type": "shader", "shader_model": {
                "parameters": [
                    {"name": "scale", "type": "float", "default": 2},
                    {"name": "tint", "type": "color", "default": {"r": 1, "g": 0, "b": 0, "a": 1}}
                ]
            }
        }"#,
        )
        .unwrap();
        let mut library = NodeLibrary::default();
        library.definitions.insert(definition.name.clone(), definition);
        let resolved = resolve_nodes(&project, &library).unwrap();

        assert!(resolved.nodes[0].shader_model.is_some());
        assert!(matches!(resolved.nodes[0].parameters["scale"], ParameterValue::Number(8.0)));
        assert!(matches!(resolved.nodes[0].parameters["tint"], ParameterValue::Color(_)));
        assert!(project.nodes[0].shader_model.is_none());

        let tagged: ParameterValue =
            serde_json::from_str(r#"{"type": "Color", "r": 0, "g": 1, "b": 0, "a": 1}"#).unwrap();
        assert!(matches!(tagged, ParameterValue::Object(ParameterObject::Color(_))));
    }
}
