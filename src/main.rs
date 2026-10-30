use std::ffi::OsStr;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use ptex::export_conditions::profile_condition_matches;
use ptex::steps::pt1_expand_graph::expand_graphs;
use ptex::steps::pt2_resolve_nodes::resolve_nodes;
use ptex::steps::pt3_plan_shader::plan_shader;
use ptex::steps::pt4_emit_node::emit_node;
use ptex::steps::pt5_assemble_shader::assemble_shader;
use ptex::structs::mmg::NodeLibrary;
use ptex::structs::ptex::{Node, Project};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("ptex: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    // Read the project path, node library directory, export profile, and output directory.
    let mut args = std::env::args().skip(1);
    let project_path = args.next().ok_or("missing project path")?;
    let nodes_dir = args.next().ok_or("missing nodes directory")?;
    let profile = args.next().ok_or("missing export profile")?;
    let output_dir = args.next().ok_or("missing output directory")?;
    if args.next().is_some() {
        return Err("too many arguments".into());
    }

    // Deserialize the .ptex project without expanding or resolving its nodes.
    let text = fs::read_to_string(&project_path)?;
    let mut project: Project = serde_json::from_str(&text)?;
    project.path = PathBuf::from(project_path);

    // Collect library files in a stable order.
    let nodes_dir = PathBuf::from(nodes_dir);
    let mut paths = Vec::new();
    for entry in fs::read_dir(&nodes_dir)? {
        let entry = entry?;
        paths.push(entry.path());
    }
    paths.sort();

    // Deserialize each .mmg definition and index it by name and filename alias.
    let mut library = NodeLibrary::default();
    for path in paths {
        if path.extension() != Some(OsStr::new("mmg")) {
            continue;
        }
        let text = fs::read_to_string(&path)?;
        let mut definition: Node = match serde_json::from_str(&text) {
            Ok(definition) => definition,
            Err(error) => return Err(format!("{}: {error}", path.display()).into()),
        };
        let filename = path.file_stem().ok_or("missing filename")?;
        let filename = filename.to_str().ok_or("filename is not UTF-8")?;
        if definition.name.is_empty() {
            definition.name = filename.to_owned();
        }
        if filename != definition.name {
            library.aliases.insert(filename.to_owned(), definition.name.clone());
        }
        library.definitions.insert(definition.name.clone(), definition);
    }

    // Load the shared shader definitions used by library nodes.
    let globals_path = nodes_dir.join("glsl_defs.tmpl");
    if globals_path.is_file() {
        library.common_globals = fs::read_to_string(&globals_path)?;
    }

    // Expand embedded and library subgraphs, apply their controls, and reconnect their ports.
    let expanded_project = expand_graphs(&project, &library)?;

    // Fill missing shader models, generic sizes, and parameter defaults from node definitions.
    let resolved_project = resolve_nodes(&expanded_project, &library)?;

    // Find the material node that collects the project's output channels.
    let mut material = None;
    for node in &resolved_project.nodes {
        if node.r#type == "material" || node.r#type == "material_unlit" {
            material = Some(node);
            break;
        }
    }
    let material = material.ok_or("project has no material node")?;

    // Read the requested export profile from the material's shader model.
    let model = material.shader_model.as_ref().ok_or("material has no shader model")?;
    let export = model.exports.get(&profile).ok_or("material has no matching export profile")?;

    // Select texture outputs whose export conditions match the connected material inputs.
    let mut maps = Vec::new();
    let expected_type = if material.r#type == "material_unlit" { "buffers" } else { "texture" };
    for file in &export.files {
        if file.r#type != expected_type || file.file_name.is_none() {
            continue;
        }
        if profile_condition_matches(&file.conditions, model, &resolved_project, material)? {
            maps.push(file);
        }
    }
    if maps.is_empty() {
        return Err(format!("profile `{profile}` has no connected texture maps").into());
    }

    // Choose a project name and replace characters unsuitable for the output filename prefix.
    let mut project_name = project.name.as_str();
    if let Some(label) = &project.label {
        if !label.is_empty() {
            project_name = label;
        }
    }
    if project_name.is_empty() {
        let stem = project.path.file_stem().ok_or("project has no filename")?;
        project_name = stem.to_str().ok_or("project filename is not UTF-8")?;
    }
    let mut prefix = String::new();
    for ch in project_name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            prefix.push(ch);
        } else {
            prefix.push('_');
        }
    }

    // Generate one shader for each selected map before writing any files.
    let mut shaders = Vec::new();
    for map in maps {
        // Unlit materials use their connected source; other materials use the profile's output index.
        let (target, output) = if material.r#type == "material_unlit" {
            let mut input = None;
            for connection in &resolved_project.connections {
                if connection.to == material.name && connection.to_port == 0 {
                    input = Some(connection);
                    break;
                }
            }
            let connection = input.ok_or("unlit material has no connected input")?;
            let mut source = None;
            for node in &resolved_project.nodes {
                if node.name == connection.from {
                    source = Some(node);
                    break;
                }
            }
            let source = source.ok_or("unlit material source does not exist")?;
            (source, connection.from_port)
        } else {
            (material, map.output)
        };

        // Plan the required outputs, their dependencies, and shared includes.
        let plan = plan_shader(&resolved_project, &library, target, output)?;

        // Expand each planned node into a shader fragment without changing the plan.
        let mut fragments = Vec::new();
        for node in &plan.nodes {
            let fragment = emit_node(node)?;
            fragments.push(fragment);
        }

        // Combine the fragments and add the shader's vertex and fragment entry points.
        let source = assemble_shader(&plan, &fragments)?;

        // Expand the export filename template and change its extension to .shader.
        let template = map.file_name.as_ref().ok_or("export map has no filename")?;
        let texture_name = template.replace("$(path_prefix)", &prefix);
        let texture_name = texture_name.replace("$(buffer_index)", "0");
        let texture_path = PathBuf::from(texture_name);
        let filename = texture_path.file_stem().ok_or("export map has an invalid filename")?;
        let filename = filename.to_str().ok_or("export filename is not UTF-8")?;
        shaders.push((format!("{filename}.shader"), source));
    }

    // Write the generated shader sources to the output directory.
    fs::create_dir_all(&output_dir)?;
    for (filename, source) in shaders {
        let path = PathBuf::from(&output_dir).join(filename);
        fs::write(&path, source)?;
        println!("{}", path.display());
    }
    Ok(())
}
