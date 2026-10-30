//! Shared shader template and literal helpers.

use crate::structs::mmg::{Color, Curve, Gradient, Output, ParameterValue, ShaderModel, Source};
use crate::structs::ptex::Node;
use crate::structs::shader::ShaderOutput;

pub(crate) fn output_name(output: &ShaderOutput) -> String {
    let name = identifier(&output.node);
    format!("{name}_output_{}", output.port)
}

pub(crate) fn source_text(source: &Source) -> String {
    match source {
        Source::Text(text) => text.clone(),
        Source::Lines(lines) => lines.join("\n"),
        Source::Empty => String::new(),
    }
}

pub(crate) fn output_expression(output: &Output) -> Option<&str> {
    let expression = match output.r#type.as_str() {
        "f" => &output.f,
        "rgb" => &output.rgb,
        "rgba" => &output.rgba,
        "sdf2d" => &output.sdf2d,
        "sdf3d" => &output.sdf3d,
        "sdf3dc" => &output.sdf3dc,
        "tex3d" => &output.tex3d,
        "tex3d_gs" => &output.tex3d_gs,
        "fill" => &output.fill,
        _ => return None,
    };
    if expression.is_some() {
        return expression.as_deref();
    }
    if output.rgba.is_some() {
        return output.rgba.as_deref();
    }
    if output.rgb.is_some() {
        return output.rgb.as_deref();
    }
    output.f.as_deref()
}

pub(crate) fn generic_input_index(model: &ShaderModel, name: &str, size: usize) -> Option<(usize, usize)> {
    for (index, input) in model.inputs.iter().enumerate() {
        let pattern = input.name.as_str();
        let Some((prefix, suffix)) = pattern.split_once('#') else {
            if pattern == name {
                return Some((index, index));
            }
            continue;
        };
        let Some(rest) = name.strip_prefix(prefix) else { continue };
        let mut digit_count = 0;
        for byte in rest.bytes() {
            if !byte.is_ascii_digit() {
                break;
            }
            digit_count += 1;
        }
        let digits = &rest[..digit_count];
        if digits.is_empty() || &rest[digit_count..] != suffix {
            continue;
        }
        let variation = digits.parse::<usize>().ok()?;
        if variation == 0 || variation > size {
            continue;
        }
        let mut group_size = 0;
        for input in &model.inputs {
            if let Some(size) = input.group_size {
                group_size = size;
                break;
            }
            if input.name.contains('#') {
                group_size += 1;
            }
        }
        return Some((index + (variation - 1) * group_size.max(1), index));
    }
    None
}

pub(crate) fn expand_generic_code(source: &str, size: usize) -> String {
    let source = source.replace("#count", &size.to_string());
    let mut result = String::new();
    let mut remaining = source.as_str();
    while let Some(start) = remaining.find("#for") {
        result.push_str(&remaining[..start]);
        let body = &remaining[start + "#for".len()..];
        let Some(end) = body.find("#end") else {
            for index in 1..=size {
                result.push_str(&body.replace('#', &index.to_string()));
            }
            return result;
        };
        for index in 1..=size {
            result.push_str(&body[..end].replace('#', &index.to_string()));
        }
        remaining = &body[end + "#end".len()..];
    }
    result.push_str(remaining);
    result
}

pub(crate) fn gradient_expression(gradient: &Gradient, value: &str) -> Result<String, String> {
    if gradient.points.is_empty() {
        return Err("gradient parameter has no points".to_owned());
    }
    let mut points = Vec::new();
    for point in &gradient.points {
        points.push((point.pos, [point.r, point.g, point.b, point.a]));
    }
    points.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut result = rgba_literal(points.last().unwrap().1);
    for (index, pair) in points.windows(2).enumerate().rev() {
        let (p0, c0) = pair[0];
        let (p1, c1) = pair[1];
        let t = format!("(({value})-{})/{}", float_literal(p0), float_literal(p1 - p0));
        let color0 = rgba_literal(c0);
        let color1 = rgba_literal(c1);
        let segment = match gradient.interpolation {
            0 => color0,
            2 => format!("mix({color0}, {color1}, 0.5-0.5*cos(3.14159265359*({t})))"),
            3 => {
                let linear = format!("mix({color0}, {color1}, {t})");
                if index > 0 {
                    let previous_position = points[index - 1].0;
                    let previous_color = rgba_literal(points[index - 1].1);
                    let a = format!(
                        "mix({previous_color}, {color0}, (({value})-{})/{})",
                        float_literal(previous_position),
                        float_literal(p0 - previous_position)
                    );
                    if index + 2 < points.len() {
                        let next_position = points[index + 2].0;
                        let next_color = rgba_literal(points[index + 2].1);
                        let c = format!(
                            "mix({color1}, {next_color}, (({value})-{})/{})",
                            float_literal(p1),
                            float_literal(next_position - p1)
                        );
                        format!("0.5*({linear}+mix({a}, {c}, 0.5-0.5*cos(3.14159265359*({t}))))")
                    } else {
                        format!("mix({a}, {linear}, 0.5+0.5*({t}))")
                    }
                } else if index + 2 < points.len() {
                    let next_position = points[index + 2].0;
                    let next_color = rgba_literal(points[index + 2].1);
                    let c = format!(
                        "mix({color1}, {next_color}, (({value})-{})/{})",
                        float_literal(p1),
                        float_literal(next_position - p1)
                    );
                    format!("mix({c}, {linear}, 1.0-0.5*({t}))")
                } else {
                    linear
                }
            }
            _ => format!("mix({color0}, {color1}, clamp({t}, 0.0, 1.0))"),
        };
        result = format!("(({value}) < {} ? {segment} : {result})", float_literal(p1));
    }
    let first_color = rgba_literal(points[0].1);
    Ok(format!("(({value}) < {} ? {first_color} : {result})", float_literal(points[0].0)))
}

pub(crate) fn curve_expression(curve: &Curve, value: &str) -> Result<String, String> {
    if curve.points.is_empty() {
        return Err("curve parameter has no points".to_owned());
    }
    let mut points = curve.points.clone();
    points.sort_by(|a, b| a.x.total_cmp(&b.x));
    let mut result = float_literal(points.last().unwrap().y);
    for pair in points.windows(2).rev() {
        let first = &pair[0];
        let second = &pair[1];
        let width = second.x - first.x;
        let t = format!("((({value})-{})/{})", float_literal(first.x), float_literal(width));
        let inverse_t = format!("(1.0-{t})");
        let y0 = float_literal(first.y);
        let y1 = float_literal(first.y + width / 3.0 * first.rs);
        let y2 = float_literal(second.y - width / 3.0 * second.ls);
        let y3 = float_literal(second.y);
        let segment = format!(
            "({y0}*({inverse_t})*({inverse_t})*({inverse_t}) + {y1}*({inverse_t})*({inverse_t})*({t})*3.0 + {y2}*({inverse_t})*({t})*({t})*3.0 + {y3}*({t})*({t})*({t}))"
        );
        result = format!("(({value}) < {} ? {segment} : {result})", float_literal(second.x));
    }
    Ok(result)
}

pub(crate) fn replace_parameter_random(source: &str, seed: f64, offset: f64) -> Result<String, String> {
    let mut result = String::new();
    let mut cursor = 0;
    loop {
        let float_random = source[cursor..].find("$rnd(");
        let integer_random = source[cursor..].find("$rndi(");
        let relative = match (float_random, integer_random) {
            (Some(a), Some(b)) => a.min(b),
            (Some(a), None) => a,
            (None, Some(b)) => b,
            (None, None) => break,
        };
        let start = cursor + relative;
        result.push_str(&source[cursor..start]);
        let name_start = start + 1;
        let open = name_start + source[name_start..].find('(').unwrap();
        let macro_name = &source[name_start..open];
        let close = matching_paren(source, open).ok_or_else(|| format!("unterminated `${macro_name}(` parameter"))?;
        let args = &source[open + 1..close];
        let random_offset = if macro_name == "rnd" {
            (start as f64 + offset).sin() * 0.5 + 0.5
        } else {
            (start as f64 + offset).sin() + offset
        };
        let function = if macro_name == "rnd" { "param_rnd" } else { "param_rndi" };
        result.push_str(&format!("{function}({args}, {})", float_literal(seed + random_offset)));
        cursor = close + 1;
    }
    result.push_str(&source[cursor..]);
    Ok(result)
}

pub(crate) fn matching_paren(source: &str, open: usize) -> Option<usize> {
    let mut depth = 0;
    for (index, byte) in source.bytes().enumerate().skip(open) {
        if byte == b'(' {
            depth += 1;
        }
        if byte == b')' {
            depth -= 1;
            if depth == 0 {
                return Some(index);
            }
        }
    }
    None
}

pub(crate) fn first_argument(source: &str) -> &str {
    let mut depth = 0;
    for (index, ch) in source.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => return source[..index].trim(),
            _ => {}
        }
    }
    source.trim()
}

pub(crate) fn convert_type(value: String, source: &str, target: &str) -> String {
    match (source, target) {
        ("sdf3dc", "sdf2d" | "sdf3d" | "f") => format!("({value}).x"),
        ("sdf2d" | "sdf3d" | "f", "sdf3dc") => format!("vec2({value}, 0.0)"),
        ("rgba", "rgb") => format!("({value}).rgb"),
        ("rgba" | "rgb", "f") => format!("({value}).r"),
        ("f", "rgb") => format!("vec3({value})"),
        ("f", "rgba") => format!("vec4(vec3({value}), 1.0)"),
        ("rgb", "rgba") => format!("vec4({value}, 1.0)"),
        _ => value,
    }
}

pub(crate) fn switch_input(node: &Node, output: usize) -> usize {
    let outputs = node.parameters.get("outputs").and_then(parameter_number).unwrap_or(1.0);
    let source = node.parameters.get("source").and_then(parameter_number).unwrap_or(0.0);
    output + source.max(0.0) as usize * outputs.max(1.0) as usize
}

pub(crate) fn parameter_number(value: &ParameterValue) -> Option<f64> {
    match value {
        ParameterValue::Number(value) => Some(*value),
        _ => None,
    }
}

pub(crate) fn scalar_literal(value: &ParameterValue) -> Result<String, String> {
    match value {
        ParameterValue::Number(value) => Ok(float_literal(*value)),
        ParameterValue::Boolean(value) => Ok(value.to_string()),
        ParameterValue::String(value) => Ok(value.clone()),
        _ => Err("expected a scalar shader parameter".to_owned()),
    }
}

pub(crate) fn color_literal(color: &Color) -> String {
    rgba_literal([color.r, color.g, color.b, color.a])
}

fn rgba_literal(color: [f64; 4]) -> String {
    let r = float_literal(color[0]);
    let g = float_literal(color[1]);
    let b = float_literal(color[2]);
    let a = float_literal(color[3]);
    format!("vec4({r}, {g}, {b}, {a})")
}

pub(crate) fn float_literal(value: f64) -> String {
    let text = value.to_string();
    if text.contains('.') || text.contains('e') || text.contains('E') { text } else { format!("{text}.0") }
}

pub(crate) fn identifier(value: &str) -> String {
    let mut name = String::from("mm_");
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            name.push(ch);
        } else {
            name.push('_');
        }
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parameter_random_macros_expand_deterministically() {
        let source = "$rnd(1.0, 2.0) + $rndi(3.0, 5.0)";
        let expanded = replace_parameter_random(source, 9.0, 17.0).unwrap();
        assert_eq!(expanded, replace_parameter_random(source, 9.0, 17.0).unwrap());
        assert!(expanded.contains("param_rnd(1.0, 2.0,"));
        assert!(expanded.contains("param_rndi(3.0, 5.0,"));
    }

    #[test]
    fn gradient_respects_constant_and_cosine_modes() {
        let mut gradient: Gradient = serde_json::from_str(
            r#"{"interpolation":0,"points":[{"pos":0,"r":1,"g":0,"b":0,"a":1},{"pos":1,"r":0,"g":0,"b":1,"a":1}]}"#,
        )
        .unwrap();
        let constant = gradient_expression(&gradient, "uv.x").unwrap();
        assert!(constant.contains("vec4(1.0, 0.0, 0.0, 1.0)"));
        gradient.interpolation = 2;
        assert!(gradient_expression(&gradient, "uv.x").unwrap().contains("cos(3.14159265359"));
    }

    #[test]
    fn curve_parameter_expands_to_cubic_segments() {
        let curve: Curve =
            serde_json::from_str(r#"{"points":[{"x":0,"y":0,"ls":0,"rs":1},{"x":1,"y":1,"ls":1,"rs":0}]}"#).unwrap();
        let expression = curve_expression(&curve, "uv.x").unwrap();
        assert!(expression.contains("uv.x"));
        assert!(expression.contains("*3.0"));
    }
}
