use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CompasDataObject {
    #[serde(rename = "dtype")]
    pub data_type: String,
    pub data: CompasDataPayload,
    #[serde(default)]
    pub guid: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CompasDataPayload {
    #[serde(default)]
    pub vertices: Option<Vec<Vec<f64>>>,
    #[serde(default)]
    pub faces: Option<Vec<Vec<usize>>>,
    #[serde(default)]
    pub attributes: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Serialize)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub vertex_count: usize,
    pub face_count: usize,
    pub non_manifold_edges: Vec<(usize, usize)>,
    pub duplicate_vertices: usize,
    pub bounding_box: crate::geometry::AABB,
}