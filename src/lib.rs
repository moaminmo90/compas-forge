use pyo3::prelude::*;
use pyo3::exceptions::PyValueError;
use rstar::{RTree, RTreeObject};
use std::sync::Mutex;

mod parser;
mod geometry;

use parser::{CompasDataObject, ValidationResult};
use geometry::{
    find_non_manifold_edges, find_boundary_edges, triangulate_face, weld_vertices, 
    unify_winding_directions, compute_mesh_volume, count_unique_edges,
    compute_max_planarity_deviation, compute_min_face_quality, AABB, SpatialPart
};
use rayon::prelude::*;
use std::collections::HashSet;

// SOTA parry3d-f64 v0.28.0 modern math types
use parry3d_f64::math::{Vector, Pose};
use parry3d_f64::shape::TriMesh;
use parry3d_f64::query::{intersection_test, distance};

#[derive(serde::Serialize)]
struct FixedMeshReport {
    welded_count: usize,
    flipped_count: usize,
    weld_details: Vec<geometry::WeldAudit>,
    flip_details: Vec<geometry::FlipAudit>,
    fixed_json: String,
}

#[derive(serde::Serialize)]
struct PreflightResult {
    profile_name: String,
    is_compliant: bool,
    volume_m3: f64,
    estimated_mass_kg: f64,
    boundary_edges_count: usize,
    boundary_edges: Vec<(usize, usize)>, 
    is_watertight: bool,
    fits_workspace: bool,
    bounds_x_dim: f64,
    bounds_y_dim: f64,
    bounds_z_dim: f64,
    bounding_box: crate::geometry::AABB,
    vertices: Vec<Vec<f64>>,
    triangulated_faces: Vec<[u32; 3]>,
    // Newly Added SOTA Scientific topological and geometric invariants
    euler_characteristic: i32,
    genus: usize,
    max_planarity_deviation: f64,
    min_face_quality: f64,
}

#[derive(serde::Serialize)]
struct AssemblyClashResult {
    part_a: String,
    part_b: String,
    has_intersection: bool,
    minimum_distance: f64,
    is_clearance_violation: bool,
}

fn check_duplicates_parallel(vertices: &[Vec<f64>]) -> usize {
    if vertices.is_empty() {
        return 0;
    }

    let signatures: Vec<String> = vertices
        .par_iter()
        .map(|v| format!("{:.6},{:.6},{:.6}", v[0], v[1], v[2]))
        .collect();

    let mut unique_set = HashSet::with_capacity(vertices.len());
    let mut duplicates = 0;

    for sig in signatures {
        if !unique_set.insert(sig) {
            duplicates += 1;
        }
    }
    duplicates
}

fn compute_mesh_distance(part_a: &SpatialPart, part_b: &SpatialPart) -> Option<f64> {
    let pts_a: Vec<Vector> = part_a.vertices
        .iter()
        .filter(|v| v.len() >= 3)
        .map(|v| Vector::new(v[0], v[1], v[2]))
        .collect();

    let pts_b: Vec<Vector> = part_b.vertices
        .iter()
        .filter(|v| v.len() >= 3)
        .map(|v| Vector::new(v[0], v[1], v[2]))
        .collect();

    let mut indices_a = Vec::new();
    for face in &part_a.faces {
        indices_a.extend(triangulate_face(face));
    }

    let mut indices_b = Vec::new();
    for face in &part_b.faces {
        indices_b.extend(triangulate_face(face));
    }

    if indices_a.is_empty() || indices_b.is_empty() {
        return None;
    }

    let mesh_a = TriMesh::new(pts_a, indices_a).ok()?;
    let mesh_b = TriMesh::new(pts_b, indices_b).ok()?;

    let pos_a = Pose::identity();
    let pos_b = Pose::identity();

    distance(&pos_a, &mesh_a, &pos_b, &mesh_b).ok()
}

#[pyfunction]
fn validate_compas_json(json_str: &str) -> PyResult<String> {
    let mut bytes = json_str.as_bytes().to_vec();

    let obj: CompasDataObject = simd_json::serde::from_slice(&mut bytes)
        .map_err(|err| PyValueError::new_err(format!("Malformed COMPAS JSON Schema (SIMD): {}", err)))?;

    let vertices = obj.data.vertices.unwrap_or_default();
    let faces = obj.data.faces.unwrap_or_default();

    let duplicate_count = check_duplicates_parallel(&vertices);
    let non_manifold = find_non_manifold_edges(&faces); 
    let bbox = AABB::from_vertices(&vertices);

    let result = ValidationResult {
        is_valid: duplicate_count == 0 && non_manifold.is_empty(),
        vertex_count: vertices.len(),
        face_count: faces.len(),
        non_manifold_edges: non_manifold,
        duplicate_vertices: duplicate_count,
        bounding_box: bbox,
    };

    serde_json::to_string(&result)
        .map_err(|err| PyValueError::new_err(format!("Failed to serialize diagnostic report: {}", err)))
}

#[pyfunction]
fn fix_mesh_json(json_str: &str) -> PyResult<String> {
    let mut bytes = json_str.as_bytes().to_vec();
    let mut obj: CompasDataObject = simd_json::serde::from_slice(&mut bytes)
        .map_err(|err| PyValueError::new_err(format!("Malformed COMPAS JSON Schema: {}", err)))?;

    let vertices = obj.data.vertices.unwrap_or_default();
    let faces = obj.data.faces.unwrap_or_default();

    let (welded_vertices, welded_faces, weld_details) = weld_vertices(&vertices, &faces);
    let (fixed_faces, flip_details) = unify_winding_directions(welded_faces);

    let welded_count = weld_details.len();
    let flipped_count = flip_details.len();

    obj.data.vertices = Some(welded_vertices);
    obj.data.faces = Some(fixed_faces);

    let fixed_json = serde_json::to_string(&obj)
        .map_err(|err| PyValueError::new_err(format!("Failed to serialize fixed mesh: {}", err)))?;

    let report = FixedMeshReport {
        welded_count,
        flipped_count,
        weld_details,
        flip_details,
        fixed_json,
    };

    serde_json::to_string(&report)
        .map_err(|err| PyValueError::new_err(format!("Failed to serialize fix report: {}", err)))
}

#[pyfunction]
fn run_preflight_json(json_str: &str, profile: &str) -> PyResult<String> {
    let mut bytes = json_str.as_bytes().to_vec();
    let obj: CompasDataObject = simd_json::serde::from_slice(&mut bytes)
        .map_err(|err| PyValueError::new_err(format!("Malformed COMPAS JSON Schema (SIMD): {}", err)))?;

    let vertices = obj.data.vertices.unwrap_or_default();
    let faces = obj.data.faces.unwrap_or_default();

    let bbox = AABB::from_vertices(&vertices);
    let volume_m3 = compute_mesh_volume(&vertices, &faces);
    let boundary_edges = find_boundary_edges(&faces);
    let boundary_edges_count = boundary_edges.len();
    let is_watertight = boundary_edges_count == 0;

    let bounds_x_dim = bbox.max_x - bbox.min_x;
    let bounds_y_dim = bbox.max_y - bbox.min_y;
    let bounds_z_dim = bbox.max_z - bbox.min_z;

    // Advanced SOTA metrics calculations
    let unique_edges_count = count_unique_edges(&faces);
    let euler_characteristic = (vertices.len() as i32) - (unique_edges_count as i32) + (faces.len() as i32);
    
    // Genus estimation for closed manifold meshes: g = (2 - chi) / 2
    let genus = if is_watertight && euler_characteristic <= 2 {
        ((2 - euler_characteristic) / 2).max(0) as usize
    } else {
        0
    };

    let max_planarity_deviation = compute_max_planarity_deviation(&vertices, &faces);
    let min_face_quality = compute_min_face_quality(&vertices, &faces);

    let mut density = 500.0;
    let mut max_mass = 150.0;
    let mut max_x = 3.0;
    let mut max_y = 0.4;
    let mut max_z = 0.4;
    let mut require_watertight = true;

    if profile == "abb-concrete-3dprint" {
        density = 2400.0;
        max_mass = 500.0;
        max_x = 1.5;
        max_y = 1.5;
        max_z = 2.0;
        require_watertight = false;
    } else if profile == "kuka-timber" {
        density = 500.0;
        max_mass = 150.0;
        max_x = 3.0;
        max_y = 0.4;
        max_z = 0.4;
        require_watertight = true;
    }

    let estimated_mass_kg = volume_m3 * density;
    let fits_workspace = bounds_x_dim <= max_x && bounds_y_dim <= max_y && bounds_z_dim <= max_z;
    
    // Planarity and facet quality constraint evaluation
    let planarity_ok = max_planarity_deviation <= 0.005; // 5mm tolerance limit
    let mesh_quality_ok = min_face_quality >= 0.1; // aspect ratio sanity check
    
    let mut is_compliant = fits_workspace && (estimated_mass_kg <= max_mass) && planarity_ok && mesh_quality_ok;
    if require_watertight && !is_watertight {
        is_compliant = false;
    }

    let mut triangulated_faces = Vec::new();
    for face in &faces {
        triangulated_faces.extend(triangulate_face(face));
    }

    let result = PreflightResult {
        profile_name: profile.to_string(),
        is_compliant,
        volume_m3,
        estimated_mass_kg,
        boundary_edges_count,
        boundary_edges, 
        is_watertight,
        fits_workspace,
        bounds_x_dim,
        bounds_y_dim,
        bounds_z_dim,
        bounding_box: bbox,
        vertices: vertices.clone(),
        triangulated_faces,
        euler_characteristic,
        genus,
        max_planarity_deviation,
        min_face_quality,
    };

    serde_json::to_string(&result)
        .map_err(|err| PyValueError::new_err(format!("Failed to serialize preflight report: {}", err)))
}

#[pyfunction]
fn detect_clashes_json(items: Vec<(String, String)>, clearance_tolerance: f64) -> PyResult<String> {
    let parts: Vec<SpatialPart> = items
        .par_iter()
        .enumerate()
        .filter_map(|(idx, (name, json_str))| {
            if let Ok(obj) = serde_json::from_str::<CompasDataObject>(json_str) {
                let vertices = obj.data.vertices.unwrap_or_default();
                let faces = obj.data.faces.unwrap_or_default();
                let bbox = AABB::from_vertices(&vertices);
                Some(SpatialPart {
                    id: idx,
                    name: name.clone(),
                    bbox,
                    vertices,
                    faces,
                })
            } else {
                None
            }
        })
        .collect();

    let rtree = RTree::bulk_load(parts.clone());
    let clash_reports = Mutex::new(Vec::new());

    parts.par_iter().for_each(|part_a| {
        let min_corner = [
            part_a.bbox.min_x - clearance_tolerance,
            part_a.bbox.min_y - clearance_tolerance,
            part_a.bbox.min_z - clearance_tolerance,
        ];
        let max_corner = [
            part_a.bbox.max_x + clearance_tolerance,
            part_a.bbox.max_y + clearance_tolerance,
            part_a.bbox.max_z + clearance_tolerance,
        ];
        let inflated_envelope = rstar::AABB::from_corners(min_corner, max_corner);

        let candidates = rtree.locate_in_envelope_intersecting(&inflated_envelope);
        for candidate in candidates {
            if part_a.id < candidate.id {
                let min_dist = compute_mesh_distance(part_a, candidate).unwrap_or(0.0);
                
                let has_intersection = min_dist <= 0.0;
                let is_clearance_violation = min_dist < clearance_tolerance;

                if has_intersection || is_clearance_violation {
                    let mut reports = clash_reports.lock().unwrap();
                    reports.push(AssemblyClashResult {
                        part_a: part_a.name.clone(),
                        part_b: candidate.name.clone(),
                        has_intersection,
                        minimum_distance: min_dist,
                        is_clearance_violation,
                    });
                }
            }
        }
    });

    let results = clash_reports.into_inner().unwrap();
    serde_json::to_string(&results)
        .map_err(|err| PyValueError::new_err(format!("Serialization error during clash phase: {}", err)))
}

#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(validate_compas_json, m)?)?;
    m.add_function(wrap_pyfunction!(fix_mesh_json, m)?)?;
    m.add_function(wrap_pyfunction!(run_preflight_json, m)?)?;
    m.add_function(wrap_pyfunction!(detect_clashes_json, m)?)?;
    Ok(())
}