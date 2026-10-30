use std::collections::BTreeSet;

use crate::shader_templates::output_name;
use crate::structs::shader::{ShaderFragment, ShaderPlan, SourceBlock};

pub fn assemble_shader(plan: &ShaderPlan, fragments: &[ShaderFragment]) -> Result<String, String> {
    let mut globals = plan.globals.clone();
    let mut functions = Vec::new();
    for node in &plan.nodes {
        let fragment = fragments.iter().find(|fragment| fragment.output == node.output);
        let Some(fragment) = fragment else {
            return Err(format!("missing fragment for node `{}` output {}", node.output.node, node.output.port));
        };
        globals.push(fragment.global.clone());
        functions.push(fragment.instance.clone());
        functions.push(fragment.function.clone());
    }

    // The plan already orders dependencies. Remove repeated definitions while preserving that order.
    let mut signatures = BTreeSet::new();
    let mut seen_sources = BTreeSet::new();
    let mut definitions = Vec::new();
    globals.extend(functions);
    for source in globals {
        if source.trim().is_empty() || !seen_sources.insert(source.clone()) {
            continue;
        }
        for block in source_blocks(&source) {
            if let Some(signature) = block.signature {
                if !signatures.insert(signature) {
                    continue;
                }
            }
            definitions.push(block.text);
        }
    }

    let target = output_name(&plan.target);
    let expression = match plan.output_type.as_str() {
        "f" => format!("vec4(({target}(uv)), ({target}(uv)), ({target}(uv)), 1.0)"),
        "rgb" => format!("vec4(({target}(uv)), 1.0)"),
        "rgba" => format!("{target}(uv)"),
        other => return Err(format!("shader target type `{other}` is unsupported")),
    };
    let definitions = definitions.join("\n");
    Ok(format!(
        r#"
render_mode cull_disabled, depth_test_disabled, blend_disabled;
varying vec2 mm_uv;
out vec4 mm_color;
{definitions}
void vertex() {{
    vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
    mm_uv = p;
    gl_Position = vec4(p * 2.0 - 1.0, 0.0, 1.0);
}}
void fragment() {{
    vec2 uv = mm_uv;
    mm_color = {expression};
}}
"#
    ))
}

fn source_blocks(source: &str) -> Vec<SourceBlock> {
    let normalized = normalize_global_braces(source);
    let mut blocks = Vec::new();
    let mut lines = normalized.lines();
    while let Some(line) = lines.next() {
        let signature = function_signature(line.trim());
        let mut text = line.to_owned();
        let mut depth = line.matches('{').count() as isize - line.matches('}').count() as isize;
        while depth > 0 {
            let Some(line) = lines.next() else { break };
            text.push('\n');
            text.push_str(line);
            depth += line.matches('{').count() as isize;
            depth -= line.matches('}').count() as isize;
        }
        blocks.push(SourceBlock { signature, text });
    }
    blocks
}

fn function_signature(line: &str) -> Option<String> {
    let (declaration, _) = line.split_once('{')?;
    if !declaration.contains('(') || !declaration.trim_end().ends_with(')') {
        return None;
    }
    let header = declaration.split('(').next()?;
    if header.split_whitespace().count() < 2 {
        return None;
    }
    let words = declaration.split_whitespace().collect::<Vec<_>>();
    Some(words.join(" "))
}

fn normalize_global_braces(source: &str) -> String {
    let mut lines = Vec::<String>::new();
    for line in source.lines() {
        if line.trim() == "{" {
            if let Some(previous) = lines.last_mut() {
                if previous.trim_end().ends_with(')') {
                    previous.push_str(" {");
                    continue;
                }
            }
        }
        lines.push(line.to_owned());
    }
    lines.join("\n")
}
