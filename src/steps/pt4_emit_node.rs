use crate::shader_templates::{
    color_literal, convert_type, curve_expression, expand_generic_code, first_argument, float_literal,
    generic_input_index, gradient_expression, identifier, matching_paren, output_expression, output_name,
    parameter_number, replace_parameter_random, scalar_literal, source_text, switch_input,
};
use crate::structs::mmg::{ParameterObject, ParameterValue};
use crate::structs::shader::{PlannedNode, ShaderFragment};

// Emit one planned output without traversing or changing other nodes.
pub fn emit_node(node: &PlannedNode) -> Result<ShaderFragment, String> {
    let function_name = output_name(&node.output);
    let return_type = match node.output_type.as_str() {
        "f" | "sdf2d" | "sdf3d" => "float",
        "rgb" => "vec3",
        "rgba" => "vec4",
        "sdf3dc" => "vec2",
        other => return Err(format!("node `{}` output type `{other}` is unsupported", node.node.name)),
    };
    let coordinate_type = match node.output_type.as_str() {
        "sdf3d" | "sdf3dc" => "vec3",
        _ => "vec2",
    };
    let mut global = String::new();
    let mut instance = String::new();
    let mut code = String::new();
    let expression;

    if node.node.r#type == "switch" || node.node.r#type == "buffer" {
        let selected = if node.node.r#type == "switch" { switch_input(&node.node, node.output.port) } else { 0 };
        if let Some(input) = node.inputs.get(&selected) {
            let source = output_name(&input.output);
            if node.node.r#type == "switch" {
                expression = format!("{source}(uv)");
            } else {
                // A buffer evaluates the source at four texel centers and interpolates the values.
                let size = node.node.parameters.get("size").and_then(parameter_number).unwrap_or(9.0);
                let resolution = float_literal(2.0_f64.powf(size));
                code = format!("vec2 texel = fract(uv) * {resolution} - vec2(0.5);\nvec2 weight = fract(texel);");
                let offsets = ["vec2(0.5)", "vec2(1.5, 0.5)", "vec2(0.5, 1.5)", "vec2(1.5)"];
                let mut samples = Vec::new();
                for offset in offsets {
                    samples.push(format!("{source}(fract((floor(texel) + {offset}) / {resolution}))"));
                }
                expression = format!(
                    "({} * (1.0 - weight.x) * (1.0 - weight.y) + {} * weight.x * (1.0 - weight.y) + {} * (1.0 - weight.x) * weight.y + {} * weight.x * weight.y)",
                    samples[0], samples[1], samples[2], samples[3]
                );
            }
        } else {
            expression = "vec4(0.0, 0.0, 0.0, 1.0)".to_owned();
        }
    } else {
        let model = node.node.shader_model.as_ref().ok_or("planned node has no shader model")?;
        let output = model.outputs.get(node.output.port).ok_or("planned node has no requested output")?;
        let output_template = output_expression(output).ok_or("planned output has no expression")?;
        let output_template = output_template.replace('#', &(node.output.port + 1).to_string());
        let size = node.node.generic_size.unwrap_or(1).max(1);

        let global_template = source_text(&model.global);
        let instance_template = expand_generic_code(&source_text(&model.instance), size);
        let code_template = expand_generic_code(&source_text(&model.code), size);
        global = expand_expression(node, &global_template, "uv", &[])?;
        instance = expand_expression(node, &instance_template, "uv", &[])?;
        code = expand_expression(node, &code_template, "uv", &[])?;
        expression = expand_expression(node, &output_template, "uv", &[])?;
    }

    let function = format!("{return_type} {function_name}({coordinate_type} uv) {{\n{code}\nreturn {expression};\n}}");
    Ok(ShaderFragment { output: node.output.clone(), global, instance, function })
}

fn expand_expression(node: &PlannedNode, source: &str, uv: &str, variables: &[String]) -> Result<String, String> {
    let model = node.node.shader_model.as_ref().ok_or("planned node has no shader model")?;
    let size = node.node.generic_size.unwrap_or(1).max(1);
    let mut result = String::new();
    let mut cursor = 0;
    while let Some(offset) = source[cursor..].find('$') {
        let start = cursor + offset;
        result.push_str(&source[cursor..start]);
        let after = start + 1;
        if source.as_bytes().get(after) == Some(&b'(') {
            let close = matching_paren(source, after).ok_or("unterminated shader variable")?;
            let variable = &source[after + 1..close];
            result.push_str(&expand_variable(node, variable, uv, variables)?);
            cursor = close + 1;
            continue;
        }
        let mut end = after;
        while end < source.len() {
            let byte = source.as_bytes()[end];
            if !byte.is_ascii_alphanumeric() && byte != b'_' && byte != b'?' && byte != b'#' {
                break;
            }
            end += 1;
        }
        if end == after {
            result.push('$');
            cursor = after;
            continue;
        }
        let name = &source[after..end];
        if name == "name" || name == "name_uv" {
            result.push_str(&expand_variable(node, name, uv, variables)?);
            cursor = end;
            continue;
        }

        let mut input = generic_input_index(model, name, size);
        if name == "input" {
            input = Some((0, 0));
        } else if let Some(index) = name.strip_prefix("input_") {
            let index = index.parse::<usize>().map_err(|e| e.to_string())?;
            input = Some((index, index));
        }
        let is_variation = input.is_some() && source[end..].starts_with(".variation(");
        if source.as_bytes().get(end) != Some(&b'(') && !is_variation {
            result.push_str(&expand_variable(node, name, uv, variables)?);
            cursor = end;
            continue;
        }

        let open = if is_variation { end + ".variation".len() } else { end };
        let close = matching_paren(source, open).ok_or_else(|| format!("unterminated `${name}(` macro"))?;
        let mut argument = &source[open + 1..close];
        if is_variation {
            argument = first_argument(argument);
        }
        let argument = expand_expression(node, argument, uv, variables)?;
        let expanded;
        if name == "uv" {
            expanded = argument;
        } else if let Some((index, definition_index)) = input {
            expanded = expand_input(node, index, definition_index, &argument, variables)?;
        } else if name == "rnd" || name == "rndi" {
            let seed = node.node.seed.or(node.node.seed_int).unwrap_or(0.0);
            let offset = ((start as f64).sin() * 0.5 + 0.5) + seed;
            let function = if name == "rnd" { "param_rnd" } else { "param_rndi" };
            expanded = format!("{function}({argument}, {})", float_literal(offset));
        } else {
            match node.node.parameters.get(name) {
                Some(ParameterValue::Object(ParameterObject::Gradient(gradient))) => {
                    expanded = gradient_expression(gradient, &argument)?;
                }
                Some(ParameterValue::Object(ParameterObject::Curve(curve))) => {
                    expanded = curve_expression(curve, &argument)?;
                }
                Some(_) => {
                    let value = expand_variable(node, name, uv, variables)?;
                    expanded = format!("{value}({argument})");
                }
                None => return Err(format!("node `{}` uses unsupported macro `${name}`", node.node.name)),
            }
        }
        result.push_str(&expanded);
        cursor = close + 1;
    }
    result.push_str(&source[cursor..]);
    Ok(result)
}

fn expand_input(
    node: &PlannedNode,
    index: usize,
    definition_index: usize,
    uv: &str,
    variables: &[String],
) -> Result<String, String> {
    let model = node.node.shader_model.as_ref().ok_or("planned node has no shader model")?;
    let input = model.inputs.get(definition_index).ok_or("shader input does not exist")?;
    if let Some(source) = node.inputs.get(&index) {
        let function = output_name(&source.output);
        return Ok(convert_type(format!("{function}({uv})"), &source.output_type, &input.r#type));
    }
    let default = input
        .default
        .as_deref()
        .ok_or_else(|| format!("node `{}` input `{}` has no connection or default", node.node.name, input.name))?;
    let variable = format!("input:{index}");
    if variables.contains(&variable) {
        return Err(format!("node `{}` has a recursive default for input {index}", node.node.name));
    }
    let mut path = variables.to_vec();
    path.push(variable);
    expand_expression(node, default, uv, &path)
}

fn expand_variable(node: &PlannedNode, variable: &str, uv: &str, variables: &[String]) -> Result<String, String> {
    let name = identifier(&node.node.name);
    let seed = node.node.seed.or(node.node.seed_int).unwrap_or(0.0);
    match variable {
        "uv" => return Ok(uv.to_owned()),
        "name" => return Ok(name),
        "name_uv" => return Ok(format!("{name}_uv")),
        "seed" => return Ok(float_literal(seed)),
        "time" | "?1" | "?2" | "?3" | "?4" => return Ok("0.0".to_owned()),
        "node_id" => return Ok(node.node_index.to_string()),
        _ => {}
    }
    let model = node.node.shader_model.as_ref().ok_or("planned node has no shader model")?;
    let parameter_name = variable.strip_prefix("param:").unwrap_or(variable);
    let mut value = node.node.parameters.get(parameter_name);
    if value.is_none() {
        if let Some(base_name) = parameter_name.strip_suffix('1') {
            value = node.node.parameters.get(base_name);
        }
    }
    let value = value.ok_or_else(|| format!("node `{}` uses unknown parameter `{parameter_name}`", node.node.name))?;
    let definition =
        model.parameters.iter().find(|parameter| parameter.name.replace('#', "1") == parameter_name.replace('#', "1"));
    let parameter_type = match definition {
        Some(definition) => definition.r#type.as_str(),
        None => "",
    };
    if variables.iter().any(|name| name == parameter_name) {
        return Err(format!("node `{}` has a recursive parameter `{parameter_name}`", node.node.name));
    }
    let mut path = variables.to_vec();
    path.push(parameter_name.to_owned());
    match parameter_type {
        "enum" => {
            let definition = definition.ok_or("enum has no definition")?;
            let selected = parameter_number(value).unwrap_or(0.0).max(0.0) as usize;
            let mut selected_value = definition.values.get(selected);
            if selected_value.is_none() {
                selected_value = definition.values.first();
            }
            let selected_value = selected_value.ok_or("enum has no values")?;
            expand_expression(node, &selected_value.value, uv, &path)
        }
        "size" => Ok(float_literal(2.0_f64.powf(parameter_number(value).unwrap_or(0.0)))),
        "color" => match value {
            ParameterValue::Object(ParameterObject::Color(color)) | ParameterValue::Color(color) => {
                Ok(color_literal(color))
            }
            _ => Err(format!("parameter `{parameter_name}` is not a color")),
        },
        "gradient" | "curve" => Err(format!("parameter `{parameter_name}` must be called")),
        "float" => {
            if let ParameterValue::String(expression) = value {
                let mut index = 0;
                for parameter in &model.parameters {
                    if parameter.r#type != "float" {
                        continue;
                    }
                    if parameter.name == parameter_name {
                        break;
                    }
                    index += 1;
                }
                let expression = replace_parameter_random(expression, seed, index as f64 * 17.0)?;
                expand_expression(node, &expression, uv, &path)
            } else {
                scalar_literal(value)
            }
        }
        "boolean" => match value {
            ParameterValue::Boolean(value) => Ok(value.to_string()),
            ParameterValue::Number(value) => Ok((*value != 0.0).to_string()),
            _ => Err(format!("parameter `{parameter_name}` is not a boolean")),
        },
        _ => scalar_literal(value),
    }
}
