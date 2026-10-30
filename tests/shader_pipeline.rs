use ptex::steps::pt3_plan_shader::plan_shader;
use ptex::steps::pt4_emit_node::emit_node;
use ptex::steps::pt5_assemble_shader::assemble_shader;
use ptex::structs::mmg::NodeLibrary;
use ptex::structs::ptex::Project;

#[test]
fn emission_resolves_enum_seed_and_typed_output() {
    let project: Project = serde_json::from_str(
        r#"{
        "nodes": [{
            "name": "source", "type": "shader", "seed_int": 7,
            "parameters": {"mode": 1},
            "shader_model": {
                "parameters": [{"name": "mode", "type": "enum", "values": [
                    {"value": "vec3(0.0)"}, {"value": "vec3(uv.x, 0.0, $seed)"}
                ]}],
                "outputs": [{"type": "rgb", "rgb": "$mode"}]
            }
        }]
    }"#,
    )
    .unwrap();
    let plan = plan_shader(&project, &NodeLibrary::default(), &project.nodes[0], 0).unwrap();
    let fragment = emit_node(&plan.nodes[0]).unwrap();
    assert!(fragment.function.contains("vec3 mm_source_output_0(vec2 uv)"));
    assert!(fragment.function.contains("return vec3(uv.x, 0.0, 7.0)"));
    let source = assemble_shader(&plan, &[fragment]).unwrap();
    assert!(source.contains("mm_color = vec4((mm_source_output_0(uv)), 1.0)"));
    assert_eq!(project.nodes[0].seed, None);
    assert_eq!(project.nodes[0].seed_int, Some(7.0));
}

#[test]
fn parameter_substitution_can_complete_a_function_name() {
    let project: Project = serde_json::from_str(
        r#"{
        "nodes": [{
            "name": "warp", "type": "shader", "parameters": {"tile": true},
            "shader_model": {
                "parameters": [{"name": "tile", "type": "boolean"}],
                "outputs": [{"type": "rgb", "rgb": "warp_$tile($uv)"}]
            }
        }]
    }"#,
    )
    .unwrap();
    let plan = plan_shader(&project, &NodeLibrary::default(), &project.nodes[0], 0).unwrap();
    let fragment = emit_node(&plan.nodes[0]).unwrap();
    assert!(fragment.function.contains("return warp_true(uv);"));
}

#[test]
fn planning_selects_referenced_outputs_and_orders_shared_dependencies() {
    let project: Project = serde_json::from_str(
        r#"{
        "nodes": [
            {"name": "target", "type": "shader", "shader_model": {
                "inputs": [
                    {"name": "left", "type": "rgb", "default": "vec3(0.0)"},
                    {"name": "right", "type": "rgb", "default": "vec3(0.0)"},
                    {"name": "unused", "type": "rgb", "default": "vec3(0.0)"}
                ],
                "outputs": [{"type": "rgb", "rgb": "$left($uv) + $right($uv)"}]
            }},
            {"name": "source", "type": "shader", "shader_model": {
                "outputs": [{"type": "f", "f": "0.5"}, {"type": "rgb", "rgb": "vec3(0.25)"}]
            }},
            {"name": "broken", "type": "unsupported"}
        ],
        "connections": [
            {"from": "source", "from_port": 1, "to": "target", "to_port": 0},
            {"from": "source", "from_port": 1, "to": "target", "to_port": 1},
            {"from": "broken", "from_port": 0, "to": "target", "to_port": 2}
        ]
    }"#,
    )
    .unwrap();
    let plan = plan_shader(&project, &NodeLibrary::default(), &project.nodes[0], 0).unwrap();
    assert_eq!(plan.nodes.len(), 2);
    assert_eq!(plan.nodes[0].output.node, "source");
    assert_eq!(plan.nodes[0].output.port, 1);
    assert_eq!(plan.nodes[1].output.node, "target");
    let mut fragments = Vec::new();
    for node in &plan.nodes {
        fragments.push(emit_node(node).unwrap());
    }
    let source = assemble_shader(&plan, &fragments).unwrap();
    assert_eq!(source.matches("vec3 mm_source_output_1(vec2 uv)").count(), 1);
    assert!(source.find("vec3 mm_source_output_1").unwrap() < source.find("vec3 mm_target_output_0").unwrap());
    assert!(!source.contains('$'));
}

#[test]
fn planning_rejects_graph_cycles() {
    let project: Project = serde_json::from_str(
        r#"{
        "nodes": [{"name": "loop", "type": "shader", "shader_model": {
            "inputs": [{"name": "in", "type": "f", "default": "0.0"}],
            "outputs": [{"type": "f", "f": "$in($uv)"}]
        }}],
        "connections": [{"from": "loop", "from_port": 0, "to": "loop", "to_port": 0}]
    }"#,
    )
    .unwrap();
    let error = plan_shader(&project, &NodeLibrary::default(), &project.nodes[0], 0).err().unwrap();
    assert!(error.contains("graph cycle"));
}

#[test]
fn assembly_deduplicates_helpers_without_changing_fragments() {
    let project: Project = serde_json::from_str(
        r#"{
        "nodes": [
            {"name": "target", "type": "shader", "shader_model": {
                "global": "float shared(float x) { return x; }",
                "inputs": [{"name": "in", "type": "f", "default": "0.0"}],
                "outputs": [{"type": "f", "f": "shared($in($uv))"}]
            }},
            {"name": "source", "type": "shader", "shader_model": {
                "global": "float shared(float x) { return x; }",
                "outputs": [{"type": "f", "f": "shared(0.5)"}]
            }}
        ],
        "connections": [{"from": "source", "from_port": 0, "to": "target", "to_port": 0}]
    }"#,
    )
    .unwrap();
    let plan = plan_shader(&project, &NodeLibrary::default(), &project.nodes[0], 0).unwrap();
    let mut fragments = Vec::new();
    for node in &plan.nodes {
        fragments.push(emit_node(node).unwrap());
    }
    let source = assemble_shader(&plan, &fragments).unwrap();
    assert_eq!(source.matches("float shared(float x)").count(), 1);
    assert_eq!(source, assemble_shader(&plan, &fragments).unwrap());
    assert_eq!(fragments[0].global, fragments[1].global);
}

#[test]
fn switches_and_buffers_preserve_the_selected_source_type() {
    let project: Project = serde_json::from_str(
        r#"{
        "nodes": [
            {"name": "cached", "type": "buffer", "parameters": {"size": 4}},
            {"name": "pick", "type": "switch", "parameters": {"source": 1, "outputs": 1}},
            {"name": "value", "type": "shader", "shader_model": {
                "outputs": [{"type": "f", "f": "uv.x"}]
            }},
            {"name": "broken", "type": "unsupported"}
        ],
        "connections": [
            {"from": "pick", "from_port": 0, "to": "cached", "to_port": 0},
            {"from": "broken", "from_port": 0, "to": "pick", "to_port": 0},
            {"from": "value", "from_port": 0, "to": "pick", "to_port": 1}
        ]
    }"#,
    )
    .unwrap();
    let plan = plan_shader(&project, &NodeLibrary::default(), &project.nodes[0], 0).unwrap();
    assert_eq!(plan.nodes.len(), 3);
    assert_eq!(plan.output_type, "f");
    let mut fragments = Vec::new();
    for node in &plan.nodes {
        assert_eq!(node.output_type, "f");
        fragments.push(emit_node(node).unwrap());
    }
    let source = assemble_shader(&plan, &fragments).unwrap();
    assert!(source.contains("float mm_cached_output_0(vec2 uv)"));
    assert!(source.contains("return mm_value_output_0(uv);"));
    assert!(source.contains("fract(uv) * 16.0"));
    assert!(!source.contains("broken"));
}

#[test]
fn planning_uses_selected_enum_dependencies_and_skips_connected_input_defaults() {
    let project: Project = serde_json::from_str(
        r#"{
        "nodes": [
            {"name": "target", "type": "shader", "parameters": {"mode": 0}, "shader_model": {
                "parameters": [{"name": "mode", "type": "enum", "values": [
                    {"value": "$left($uv)"}, {"value": "$unused($uv)"}
                ]}],
                "inputs": [
                    {"name": "left", "type": "f", "default": "$unused($uv)"},
                    {"name": "unused", "type": "f", "default": "0.0"}
                ],
                "outputs": [{"type": "f", "f": "$mode"}]
            }},
            {"name": "source", "type": "shader", "shader_model": {
                "outputs": [{"type": "f", "f": "0.5"}]
            }},
            {"name": "broken", "type": "unsupported"}
        ],
        "connections": [
            {"from": "source", "from_port": 0, "to": "target", "to_port": 0},
            {"from": "broken", "from_port": 0, "to": "target", "to_port": 1}
        ]
    }"#,
    )
    .unwrap();
    let plan = plan_shader(&project, &NodeLibrary::default(), &project.nodes[0], 0).unwrap();
    assert_eq!(plan.nodes.len(), 2);
    let fragment = emit_node(&plan.nodes[1]).unwrap();
    assert!(fragment.function.contains("return mm_source_output_0(uv);"));
}

#[test]
fn includes_are_ordered_deduplicated_and_checked_for_cycles() {
    let project: Project = serde_json::from_str(
        r#"{
        "nodes": [{"name": "target", "type": "shader", "shader_model": {
            "includes": ["a", "a"],
            "outputs": [{"type": "f", "f": "helper_a()"}]
        }}]
    }"#,
    )
    .unwrap();
    let a = serde_json::from_str(
        r#"{
        "name": "a", "type": "shader", "shader_model": {
            "includes": ["b"], "global": "float helper_a() { return helper_b(); }"
        }
    }"#,
    )
    .unwrap();
    let b = serde_json::from_str(
        r#"{
        "name": "b", "type": "shader", "shader_model": {
            "global": "float helper_b() { return 0.5; }"
        }
    }"#,
    )
    .unwrap();
    let mut library = NodeLibrary::default();
    library.definitions.insert("a".to_owned(), a);
    library.definitions.insert("b".to_owned(), b);
    let plan = plan_shader(&project, &library, &project.nodes[0], 0).unwrap();
    assert_eq!(plan.globals.len(), 3);
    assert!(plan.globals[1].contains("helper_b"));
    assert!(plan.globals[2].contains("helper_a"));
    let fragment = emit_node(&plan.nodes[0]).unwrap();
    let source = assemble_shader(&plan, &[fragment]).unwrap();
    assert_eq!(source.matches("float helper_a()").count(), 1);
    assert_eq!(source.matches("float helper_b()").count(), 1);

    let b = library.definitions.get_mut("b").unwrap();
    b.shader_model.as_mut().unwrap().includes.push("a".to_owned());
    let error = plan_shader(&project, &library, &project.nodes[0], 0).err().unwrap();
    assert!(error.contains("include cycle"));
}
