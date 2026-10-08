use serde::{Deserialize, Serialize};

pub type NodeId = u32;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExternalReference {
    pub method_full_name: String,
    pub node: NodeId,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    pub kind: String,
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method_full_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_ref: Option<ExternalReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cfg_nop: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_name: Option<String>,
    pub line: i64,
    pub column: usize,
    pub start_byte: usize,
    pub end_byte: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct Edge {
    pub source: NodeId,
    pub target: NodeId,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PropertyGraph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BasicBlock {
    pub id: NodeId,
    pub statements: Vec<NodeId>,
    pub is_entrypoint: bool,
    pub is_exitpoint: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Cfg {
    pub nodes: Vec<BasicBlock>,
    pub edges: Vec<[NodeId; 2]>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DefinitionSet {
    pub node: NodeId,
    pub incoming: Vec<NodeId>,
    pub outgoing: Vec<NodeId>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FunctionGraph {
    pub name: String,
    pub fullname: String,
    pub filename: String,
    pub return_type: String,
    pub signature: String,
    pub start_line: i64,
    pub end_line: i64,
    pub cpg: PropertyGraph,
    pub cfg: Cfg,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ddg: Option<PropertyGraph>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ddg_projection: Option<PropertyGraph>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reaching_definitions: Option<Vec<DefinitionSet>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Diagnostic {
    pub filename: String,
    pub line: usize,
    pub column: usize,
    pub severity: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Analysis {
    pub schema_version: u32,
    pub functions: Vec<FunctionGraph>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Options {
    pub language: Option<String>,
    pub preprocessed: bool,
    pub data_flow: bool,
    pub reaching_definitions: bool,
    pub strict: bool,
}
