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
    // COMPAS 1.x schema representations (Plural)
    #[serde(default)]
    pub vertices: Option<Vec<Vec<f64>>>,
    #[serde(default)]
    pub faces: Option<Vec<Vec<usize>>>,

    // COMPAS 2.x schema representations (Singular map structures)
    #[serde(default)]
    pub vertex: Option<HashMap<String, HashMap<String, f64>>>,
    #[serde(default)]
    pub face: Option<HashMap<String, Vec<usize>>>,

    #[serde(default)]
    pub attributes: Option<HashMap<String, serde_json::Value>>,
}

impl CompasDataPayload {
    /// Dynamically resolves and unifies the mesh geometry representation
    /// across both COMPAS 1.x and COMPAS 2.x database schemas.
    pub fn get_vertices_and_faces(&self) -> (Vec<Vec<f64>>, Vec<Vec<usize>>) {
        let mut out_vertices = Vec::new();
        let mut out_faces = Vec::new();

        // 1. Resolve and reconstruct vertices
        if let Some(ref v) = self.vertices {
            out_vertices = v.clone();
        } else if let Some(ref v_map) = self.vertex {
            let mut keys: Vec<usize> = v_map.keys()
                .filter_map(|k| k.parse::<usize>().ok())
                .collect();
            keys.sort_unstable();
            
            out_vertices = Vec::with_capacity(keys.len());
            for k in keys {
                if let Some(coords) = v_map.get(&k.to_string()) {
                    let x = coords.get("x").copied().unwrap_or(0.0);
                    let y = coords.get("y").copied().unwrap_or(0.0);
                    let z = coords.get("z").copied().unwrap_or(0.0);
                    out_vertices.push(vec![x, y, z]);
                }
            }
        }

        // 2. Resolve and reconstruct faces
        if let Some(ref f) = self.faces {
            out_faces = f.clone();
        } else if let Some(ref f_map) = self.face {
            let mut keys: Vec<usize> = f_map.keys()
                .filter_map(|k| k.parse::<usize>().ok())
                .collect();
            keys.sort_unstable();
            
            out_faces = Vec::with_capacity(keys.len());
            for k in keys {
                if let Some(f_idx) = f_map.get(&k.to_string()) {
                    out_faces.push(f_idx.clone());
                }
            }
        }

        (out_vertices, out_faces)
    }
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