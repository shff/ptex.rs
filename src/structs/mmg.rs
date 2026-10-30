use std::collections::BTreeMap;

use serde::Deserialize;

use crate::structs::ptex::Node;

#[derive(Default)]
pub struct NodeLibrary {
    pub definitions: BTreeMap<String, Node>,
    pub aliases: BTreeMap<String, String>,
    pub common_globals: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ShaderModel {
    pub name: String,
    pub shortdesc: String,
    pub longdesc: String,
    pub global: Source,
    pub instance: Source,
    pub code: Source,
    pub includes: Vec<String>,
    pub inputs: Vec<Input>,
    pub outputs: Vec<Output>,
    pub parameters: Vec<Parameter>,
    pub exports: BTreeMap<String, ExportProfile>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Input {
    pub name: String,
    pub r#type: String,
    pub default: Option<String>,
    pub group_size: Option<usize>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Output {
    pub r#type: String,
    pub f: Option<String>,
    pub rgb: Option<String>,
    pub rgba: Option<String>,
    pub sdf2d: Option<String>,
    pub sdf3d: Option<String>,
    pub sdf3dc: Option<String>,
    pub tex3d: Option<String>,
    pub tex3d_gs: Option<String>,
    pub fill: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Parameter {
    pub name: String,
    pub r#type: String,
    pub default: Option<ParameterValue>,
    #[serde(default)]
    pub values: Vec<EnumValue>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct EnumValue {
    pub name: Option<String>,
    pub value: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ExportProfile {
    pub files: Vec<ExportFile>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ExportFile {
    pub r#type: String,
    pub file_name: Option<String>,
    #[serde(default)]
    pub conditions: String,
    #[serde(default)]
    pub output: usize,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(untagged)]
pub enum Source {
    Text(String),
    Lines(Vec<String>),
    #[default]
    Empty,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum ParameterValue {
    Number(f64),
    Boolean(bool),
    String(String),
    Object(ParameterObject),
    Color(Color),
    Null,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ParameterObject {
    Color(Color),
    Gradient(Gradient),
    Curve(Curve),
    Polygon { points: Vec<Point> },
    Splines { point_groups: Vec<Vec<usize>>, values: Vec<f64> },
    Pixels { bpp: usize, h: usize, w: usize, pl: Vec<Color>, px: Vec<u64> },
    Lattice { points: Vec<Point>, size: Point },
}

#[derive(Clone, Debug, Deserialize)]
pub struct Color {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Gradient {
    pub interpolation: u32,
    pub points: Vec<GradientPoint>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GradientPoint {
    pub pos: f64,
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Curve {
    pub points: Vec<CurvePoint>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CurvePoint {
    pub x: f64,
    pub y: f64,
    pub ls: f64,
    pub rs: f64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}
