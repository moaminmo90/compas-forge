use pyo3::prelude::*;
use pyo3::exceptions::PyValueError;
use pyo3::buffer::PyBuffer;
use pyo3::types::PyDict;
use rstar::RTree;
use std::sync::Mutex;
use std::collections::HashMap;
use std::sync::OnceLock;

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

use parry3d_f64::math::{Vector, Pose, Rotation};
use parry3d_f64::shape::TriMesh;
use parry3d_f64::query::{cast_shapes, distance, ShapeCastOptions};

static MESH_REGISTRY: OnceLock<Mutex<HashMap<String, TriMesh>>> = OnceLock::new();

fn get_mesh_registry() -> &'static Mutex<HashMap<String, TriMesh>> {
    MESH_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(serde::Serialize)]
struct FixedMeshReport {
    welded_count: usize,
    flipped_count: usize,
    weld_details: Vec<geometry::WeldAudit>,
    flip_details: Vec<geometry::FlipAudit>,
    fixed_json: String,
}

#[derive(serde::Serialize)]
struct FixedBuffersReport {
    vertices: Vec<f64>,
    face_indices: Vec<i32>,
    face_offsets: Vec<i32>,
    welded_count: usize,
    flipped_count: usize,
    weld_details: Vec<geometry::WeldAudit>,
    flip_details: Vec<geometry::FlipAudit>,
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

#[derive(serde::Serialize)]
struct SweptCollisionResult {
    has_collision: bool,
    time_of_impact: f64,
    witness_a: Vec<f64>,
    witness_b: Vec<f64>,
    normal_a: Vec<f64>,
}

#[derive(serde::Serialize)]
struct ContactInterface {
    block_a: String,
    block_b: String,
    area_m2: f64,
    centroid: [f64; 3],
    normal: [f64; 3],
    vertices_3d: Vec<[f64; 3]>,
}

#[derive(Debug, Clone, Copy)]
struct Point2D {
    x: f64,
    y: f64,
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

fn parse_pose(arr: &[f64]) -> PyResult<Pose> {
    if arr.len() != 7 {
        return Err(PyValueError::new_err(
            "Pose array must contain exactly 7 elements: [x, y, z, qx, qy, qz, qw]"
        ));
    }
    let translation = Vector::new(arr[0], arr[1], arr[2]);
    let rotation = Rotation::from_xyzw(arr[3], arr[4], arr[5], arr[6]);
    Ok(Pose { rotation, translation })
}

fn trimesh_from_buffers(
    vertices_flat: &[f64],
    face_indices: &[i32],
    face_offsets: &[i32],
) -> PyResult<TriMesh> {
    let vertex_count = vertices_flat.len() / 3;
    let mut vertices = Vec::with_capacity(vertex_count);
    for chunk in vertices_flat.chunks_exact(3) {
        vertices.push(Vector::new(chunk[0], chunk[1], chunk[2]));
    }

    let face_count = if face_offsets.is_empty() { 0 } else { face_offsets.len() - 1 };
    let mut indices = Vec::with_capacity(face_count);
    for i in 0..face_count {
        let start = face_offsets[i] as usize;
        let end = face_offsets[i + 1] as usize;
        let mut face_verts = Vec::with_capacity(end - start);
        for &idx in &face_indices[start..end] {
            face_verts.push(idx as usize);
        }
        let tris = triangulate_face(&face_verts);
        for tri in tris {
            indices.push(tri);
        }
    }

    TriMesh::new(vertices, indices)
        .map_err(|e| PyValueError::new_err(format!("Failed to build TriMesh structure: {}", e)))
}

fn interpolate_pose(start: &Pose, end: &Pose, t: f64) -> Pose {
    let translation = start.translation.lerp(end.translation, t);
    let rotation = start.rotation.slerp(end.rotation, t);
    Pose { rotation, translation }
}

fn is_inside(p: Point2D, cp1: Point2D, cp2: Point2D) -> bool {
    (cp2.x - cp1.x) * (p.y - cp1.y) - (cp2.y - cp1.y) * (p.x - cp1.x) >= -1e-9
}

fn intersection_point(s: Point2D, p: Point2D, cp1: Point2D, cp2: Point2D) -> Option<Point2D> {
    let dc = Point2D { x: cp1.x - cp2.x, y: cp1.y - cp2.y };
    let dp = Point2D { x: s.x - p.x, y: s.y - p.y };
    let n1 = cp1.x * cp2.y - cp1.y * cp2.x;
    let n2 = s.x * p.y - s.y * p.x;
    let num = n1 * dp.x - dc.x * n2;
    let den = dc.x * dp.y - dc.y * dp.x;
    if den.abs() < 1e-12 {
        None
    } else {
        Some(Point2D {
            x: num / den,
            y: (n1 * dp.y - dc.y * n2) / den,
        })
    }
}

fn clip_polygon(subject: &[Point2D], clip: &[Point2D]) -> Vec<Point2D> {
    let mut output = subject.to_vec();
    let len = clip.len();
    if len < 3 { return Vec::new(); }
    
    for i in 0..len {
        let cp1 = clip[i];
        let cp2 = clip[(i + 1) % len];
        let input = output;
        output = Vec::new();
        if input.is_empty() { break; }
        
        let mut s = input[input.len() - 1];
        for &p in &input {
            if is_inside(p, cp1, cp2) {
                if !is_inside(s, cp1, cp2) {
                    if let Some(intersection) = intersection_point(s, p, cp1, cp2) {
                        output.push(intersection);
                    }
                }
                output.push(p);
            } else if is_inside(s, cp1, cp2) {
                if let Some(intersection) = intersection_point(s, p, cp1, cp2) {
                    output.push(intersection);
                }
            }
            s = p;
        }
    }
    output
}

fn polygon_area_and_centroid(poly: &[Point2D]) -> (f64, Point2D) {
    let n = poly.len();
    if n < 3 { return (0.0, Point2D { x: 0.0, y: 0.0 }); }
    let mut area = 0.0;
    let mut cx = 0.0;
    let mut cy = 0.0;
    for i in 0..n {
        let p1 = poly[i];
        let p2 = poly[(i + 1) % n];
        let factor = p1.x * p2.y - p2.x * p1.y;
        area += factor;
        cx += (p1.x + p2.x) * factor;
        cy += (p1.y + p2.y) * factor;
    }
    area = area / 2.0;
    if area.abs() < 1e-9 {
        (0.0, Point2D { x: 0.0, y: 0.0 })
    } else {
        let area_abs = area.abs();
        let factor = 6.0 * area;
        (area_abs, Point2D { x: cx / factor, y: cy / factor })
    }
}

fn calculate_face_normal_and_centroid(vertices: &[Vector], face: &[usize]) -> (Vector, Vector) {
    let len = face.len();
    let mut centroid = Vector::new(0.0, 0.0, 0.0);
    for &idx in face {
        centroid += vertices[idx];
    }
    centroid /= len as f64;

    let mut normal = Vector::new(0.0, 0.0, 0.0);
    for i in 0..len {
        let vi = vertices[face[i]];
        let vj = vertices[face[(i + 1) % len]];
        normal.x += (vi.y - vj.y) * (vi.z + vj.z);
        normal.y += (vi.z - vj.z) * (vi.x + vj.x);
        normal.z += (vi.x - vj.x) * (vi.y + vj.y);
    }
    if normal.length_squared() > 1e-12 {
        normal = normal.normalize();
    }
    (normal, centroid)
}

#[pyfunction]
fn validate_compas_json(json_str: &str) -> PyResult<String> {
    let mut bytes = json_str.as_bytes().to_vec();

    let obj: CompasDataObject = simd_json::serde::from_slice(&mut bytes)
        .map_err(|err| PyValueError::new_err(format!("Malformed COMPAS JSON Schema (SIMD): {}", err)))?;

    let (vertices, faces) = obj.data.get_vertices_and_faces();

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
fn validate_mesh_buffers(
    py: Python<'_>,
    vertices: PyBuffer<f64>,
    face_indices: PyBuffer<i32>,
    face_offsets: PyBuffer<i32>,
) -> PyResult<String> {
    let v_slice = vertices.as_slice(py)
        .ok_or_else(|| PyValueError::new_err("Vertices buffer must be contiguous and flat"))?;
    let idx_slice = face_indices.as_slice(py)
        .ok_or_else(|| PyValueError::new_err("Face indices buffer must be contiguous and flat"))?;
    let off_slice = face_offsets.as_slice(py)
        .ok_or_else(|| PyValueError::new_err("Face offsets buffer must be contiguous and flat"))?;

    let vertices_flat: &[f64] = unsafe { std::mem::transmute(v_slice) };
    let face_indices: &[i32] = unsafe { std::mem::transmute(idx_slice) };
    let face_offsets: &[i32] = unsafe { std::mem::transmute(off_slice) };

    let vertex_count = vertices_flat.len() / 3;
    let mut vertices = Vec::with_capacity(vertex_count);
    for chunk in vertices_flat.chunks_exact(3) {
        vertices.push(vec![chunk[0], chunk[1], chunk[2]]);
    }

    let face_count = if face_offsets.is_empty() { 0 } else { face_offsets.len() - 1 };
    let mut faces = Vec::with_capacity(face_count);
    for i in 0..face_count {
        let start = face_offsets[i] as usize;
        let end = face_offsets[i + 1] as usize;
        let mut face = Vec::with_capacity(end - start);
        for &idx in &face_indices[start..end] {
            face.push(idx as usize);
        }
        faces.push(face);
    }

    let duplicate_count = check_duplicates_parallel(&vertices);
    let non_manifold = find_non_manifold_edges(&faces); 
    let bbox = AABB::from_vertices(&vertices);

    let result = ValidationResult {
        is_valid: duplicate_count == 0 && non_manifold.is_empty(),
        vertex_count,
        face_count,
        non_manifold_edges: non_manifold,
        duplicate_vertices: duplicate_count,
        bounding_box: bbox,
    };

    serde_json::to_string(&result)
        .map_err(|err| PyValueError::new_err(format!("Failed to serialize zero-copy diagnostic report: {}", err)))
}

#[pyfunction]
fn check_swept_collision(
    py: Python<'_>,
    v1_obj: &Bound<'_, PyAny>,
    idx1_obj: &Bound<'_, PyAny>,
    off1_obj: &Bound<'_, PyAny>,
    pose1_start_vec: Vec<f64>,
    pose1_end_vec: Vec<f64>,
    v2_obj: &Bound<'_, PyAny>,
    idx2_obj: &Bound<'_, PyAny>,
    off2_obj: &Bound<'_, PyAny>,
    pose2_start_vec: Vec<f64>,
    pose2_end_vec: Vec<f64>,
) -> PyResult<String> {
    let v1_buf = PyBuffer::<f64>::get(v1_obj).map_err(|e| PyValueError::new_err(format!("v1 buffer err: {}", e)))?;
    let idx1_buf = PyBuffer::<i32>::get(idx1_obj).map_err(|e| PyValueError::new_err(format!("idx1 buffer err: {}", e)))?;
    let off1_buf = PyBuffer::<i32>::get(off1_obj).map_err(|e| PyValueError::new_err(format!("off1 buffer err: {}", e)))?;

    let v2_buf = PyBuffer::<f64>::get(v2_obj).map_err(|e| PyValueError::new_err(format!("v2 buffer err: {}", e)))?;
    let idx2_buf = PyBuffer::<i32>::get(idx2_obj).map_err(|e| PyValueError::new_err(format!("idx2 buffer err: {}", e)))?;
    let off2_buf = PyBuffer::<i32>::get(off2_obj).map_err(|e| PyValueError::new_err(format!("off2 buffer err: {}", e)))?;

    let v1_slice = v1_buf.as_slice(py).ok_or_else(|| PyValueError::new_err("v1 not contiguous"))?;
    let idx1_slice = idx1_buf.as_slice(py).ok_or_else(|| PyValueError::new_err("idx1 not contiguous"))?;
    let off1_slice = off1_buf.as_slice(py).ok_or_else(|| PyValueError::new_err("off1 not contiguous"))?;

    let v2_slice = v2_buf.as_slice(py).ok_or_else(|| PyValueError::new_err("v2 not contiguous"))?;
    let idx2_slice = idx2_buf.as_slice(py).ok_or_else(|| PyValueError::new_err("idx2 not contiguous"))?;
    let off2_slice = off2_buf.as_slice(py).ok_or_else(|| PyValueError::new_err("off2 not contiguous"))?;

    let v1_flat: &[f64] = unsafe { std::mem::transmute(v1_slice) };
    let idx1: &[i32] = unsafe { std::mem::transmute(idx1_slice) };
    let off1: &[i32] = unsafe { std::mem::transmute(off1_slice) };

    let v2_flat: &[f64] = unsafe { std::mem::transmute(v2_slice) };
    let idx2: &[i32] = unsafe { std::mem::transmute(idx2_slice) };
    let off2: &[i32] = unsafe { std::mem::transmute(off2_slice) };

    let p1_start = parse_pose(&pose1_start_vec)?;
    let p1_end = parse_pose(&pose1_end_vec)?;
    let p2_start = parse_pose(&pose2_start_vec)?;
    let p2_end = parse_pose(&pose2_end_vec)?;

    let mesh1 = trimesh_from_buffers(v1_flat, idx1, off1)?;
    let mesh2 = trimesh_from_buffers(v2_flat, idx2, off2)?;

    let options = ShapeCastOptions::default();
    
    // Rotational SLERP/LERP Sub-stepping solver (10 Piecewise Linear Steps)
    let num_steps = 10;
    let mut final_hit = None;

    for step in 0..num_steps {
        let t_start = step as f64 / num_steps as f64;
        let t_end = (step + 1) as f64 / num_steps as f64;

        let p1_step_start = interpolate_pose(&p1_start, &p1_end, t_start);
        let p1_step_end = interpolate_pose(&p1_start, &p1_end, t_end);
        let p2_step_start = interpolate_pose(&p2_start, &p2_end, t_start);
        let p2_step_end = interpolate_pose(&p2_start, &p2_end, t_end);

        let step_vel1 = p1_step_end.translation - p1_step_start.translation;
        let step_vel2 = p2_step_end.translation - p2_step_start.translation;

        if let Ok(Some(hit)) = cast_shapes(
            &p1_step_start,
            step_vel1,
            &mesh1,
            &p2_step_start,
            step_vel2,
            &mesh2,
            options,
        ) {
            let actual_toi = t_start + hit.time_of_impact * (t_end - t_start);
            final_hit = Some(SweptCollisionResult {
                has_collision: true,
                time_of_impact: actual_toi,
                witness_a: vec![hit.witness1[0], hit.witness1[1], hit.witness1[2]],
                witness_b: vec![hit.witness2[0], hit.witness2[1], hit.witness2[2]],
                normal_a: vec![hit.normal1[0], hit.normal1[1], hit.normal1[2]],
            });
            break;
        }
    }

    let res = final_hit.unwrap_or(SweptCollisionResult {
        has_collision: false,
        time_of_impact: 1.0,
        witness_a: vec![0.0; 3],
        witness_b: vec![0.0; 3],
        normal_a: vec![0.0; 3],
    });

    serde_json::to_string(&res).map_err(|e| PyValueError::new_err(format!("Serialization error: {}", e)))
}

#[pyfunction]
fn register_mesh(
    mesh_id: String,
    vertices_flat: Vec<f64>,
    face_indices: Vec<i32>,
    face_offsets: Vec<i32>,
) -> PyResult<String> {
    let trimesh = trimesh_from_buffers(&vertices_flat, &face_indices, &face_offsets)?;
    let registry = get_mesh_registry();
    let mut guard = registry.lock().map_err(|e| {
        PyValueError::new_err(format!("Failed to acquire mesh registry lock: {}", e))
    })?;
    guard.insert(mesh_id.clone(), trimesh);
    Ok(format!("Mesh '{}' successfully registered.", mesh_id))
}

#[pyfunction]
fn clear_mesh_registry() -> PyResult<String> {
    let registry = get_mesh_registry();
    let mut guard = registry.lock().map_err(|e| {
        PyValueError::new_err(format!("Failed to acquire mesh registry lock: {}", e))
    })?;
    guard.clear();
    Ok("Mesh registry cleared successfully.".to_string())
}

#[pyfunction]
fn check_swept_collision_cached(
    mesh1_id: String,
    pose1_start_vec: Vec<f64>,
    pose1_end_vec: Vec<f64>,
    mesh2_id: String,
    pose2_start_vec: Vec<f64>,
    pose2_end_vec: Vec<f64>,
) -> PyResult<String> {
    let p1_start = parse_pose(&pose1_start_vec)?;
    let p1_end = parse_pose(&pose1_end_vec)?;
    let p2_start = parse_pose(&pose2_start_vec)?;
    let p2_end = parse_pose(&pose2_end_vec)?;

    let registry = get_mesh_registry();
    let guard = registry.lock().map_err(|e| {
        PyValueError::new_err(format!("Failed to acquire mesh registry lock: {}", e))
    })?;

    let mesh1 = guard.get(&mesh1_id).ok_or_else(|| {
        PyValueError::new_err(format!(
            "Mesh ID '{}' not found in registry. Please register it first.",
            mesh1_id
        ))
    })?;

    let mesh2 = guard.get(&mesh2_id).ok_or_else(|| {
        PyValueError::new_err(format!(
            "Mesh ID '{}' not found in registry. Please register it first.",
            mesh2_id
        ))
    })?;

    let options = ShapeCastOptions::default();
    let num_steps = 10;
    let mut final_hit = None;

    for step in 0..num_steps {
        let t_start = step as f64 / num_steps as f64;
        let t_end = (step + 1) as f64 / num_steps as f64;

        let p1_step_start = interpolate_pose(&p1_start, &p1_end, t_start);
        let p1_step_end = interpolate_pose(&p1_start, &p1_end, t_end);
        let p2_step_start = interpolate_pose(&p2_start, &p2_end, t_start);
        let p2_step_end = interpolate_pose(&p2_start, &p2_end, t_end);

        let step_vel1 = p1_step_end.translation - p1_step_start.translation;
        let step_vel2 = p2_step_end.translation - p2_step_start.translation;

        if let Ok(Some(hit)) = cast_shapes(
            &p1_step_start,
            step_vel1,
            mesh1,
            &p2_step_start,
            step_vel2,
            mesh2,
            options,
        ) {
            let actual_toi = t_start + hit.time_of_impact * (t_end - t_start);
            final_hit = Some(SweptCollisionResult {
                has_collision: true,
                time_of_impact: actual_toi,
                witness_a: vec![hit.witness1[0], hit.witness1[1], hit.witness1[2]],
                witness_b: vec![hit.witness2[0], hit.witness2[1], hit.witness2[2]],
                normal_a: vec![hit.normal1[0], hit.normal1[1], hit.normal1[2]],
            });
            break;
        }
    }

    let res = final_hit.unwrap_or(SweptCollisionResult {
        has_collision: false,
        time_of_impact: 1.0,
        witness_a: vec![0.0; 3],
        witness_b: vec![0.0; 3],
        normal_a: vec![0.0; 3],
    });

    serde_json::to_string(&res).map_err(|e| PyValueError::new_err(format!("Serialization error: {}", e)))
}

#[pyfunction]
fn compute_assembly_contacts(
    py: Python<'_>,
    assembly_list: Vec<Bound<'_, PyDict>>,
    tolerance: f64,
) -> PyResult<String> {
    struct MeshReconstruction {
        name: String,
        vertices: Vec<Vector>,
        faces: Vec<Vec<usize>>,
        bbox: AABB,
    }

    let mut meshes = Vec::with_capacity(assembly_list.len());

    for dict_bound in assembly_list {
        let name: String = dict_bound.get_item("name")?.unwrap().extract()?;
        let v_obj = dict_bound.get_item("vertices")?.unwrap();
        let idx_obj = dict_bound.get_item("indices")?.unwrap();
        let off_obj = dict_bound.get_item("offsets")?.unwrap();

        let v_buf = PyBuffer::<f64>::get(&v_obj).map_err(|e| PyValueError::new_err(format!("v buffer: {}", e)))?;
        let idx_buf = PyBuffer::<i32>::get(&idx_obj).map_err(|e| PyValueError::new_err(format!("idx buffer: {}", e)))?;
        let off_buf = PyBuffer::<i32>::get(&off_obj).map_err(|e| PyValueError::new_err(format!("off buffer: {}", e)))?;

        let v_slice = v_buf.as_slice(py).ok_or_else(|| PyValueError::new_err("v not contiguous"))?;
        let idx_slice = idx_buf.as_slice(py).ok_or_else(|| PyValueError::new_err("idx not contiguous"))?;
        let off_slice = off_buf.as_slice(py).ok_or_else(|| PyValueError::new_err("off not contiguous"))?;

        let v_flat: &[f64] = unsafe { std::mem::transmute(v_slice) };
        let idx: &[i32] = unsafe { std::mem::transmute(idx_slice) };
        let off: &[i32] = unsafe { std::mem::transmute(off_slice) };

        let vertex_count = v_flat.len() / 3;
        let mut vertices = Vec::with_capacity(vertex_count);
        let mut raw_v_vec = Vec::with_capacity(vertex_count);
        for chunk in v_flat.chunks_exact(3) {
            vertices.push(Vector::new(chunk[0], chunk[1], chunk[2]));
            raw_v_vec.push(vec![chunk[0], chunk[1], chunk[2]]);
        }

        let face_count = if off.is_empty() { 0 } else { off.len() - 1 };
        let mut faces = Vec::with_capacity(face_count);
        for i in 0..face_count {
            let start = off[i] as usize;
            let end = off[i + 1] as usize;
            let mut face = Vec::with_capacity(end - start);
            for &idx_val in &idx[start..end] {
                face.push(idx_val as usize);
            }
            faces.push(face);
        }

        let bbox = AABB::from_vertices(&raw_v_vec);

        meshes.push(MeshReconstruction {
            name,
            vertices,
            faces,
            bbox,
        });
    }

    let contact_interfaces = Mutex::new(Vec::new());

    (0..meshes.len()).into_par_iter().for_each(|i| {
        for j in (i + 1)..meshes.len() {
            let mesh_a = &meshes[i];
            let mesh_b = &meshes[j];

            let dx = (mesh_a.bbox.min_x - mesh_b.bbox.max_x).max(mesh_b.bbox.min_x - mesh_a.bbox.max_x);
            let dy = (mesh_a.bbox.min_y - mesh_b.bbox.max_y).max(mesh_b.bbox.min_y - mesh_a.bbox.max_y);
            let dz = (mesh_a.bbox.min_z - mesh_b.bbox.max_z).max(mesh_b.bbox.min_z - mesh_a.bbox.max_z);

            if dx <= tolerance && dy <= tolerance && dz <= tolerance {
                for f_a in &mesh_a.faces {
                    let (n_a, c_a) = calculate_face_normal_and_centroid(&mesh_a.vertices, f_a);
                    if n_a.length_squared() < 1e-6 { continue; }

                    for f_b in &mesh_b.faces {
                        let (n_b, c_b) = calculate_face_normal_and_centroid(&mesh_b.vertices, f_b);
                        if n_b.length_squared() < 1e-6 { continue; }

                        let normal_dot = n_a.dot(n_b);
                        if normal_dot < -0.95 {
                            // Enhanced proximity detection: allows non-planar/warped centroids within tolerance bounds
                            let dist = (c_a - c_b).dot(n_a).abs();
                            if dist <= tolerance {
                                let u = if n_a.x.abs() > 0.1 {
                                    Vector::new(-n_a.y, n_a.x, 0.0).normalize()
                                } else {
                                    Vector::new(0.0, -n_a.z, n_a.y).normalize()
                                };
                                let v = n_a.cross(u).normalize();

                                let poly_a_2d: Vec<Point2D> = f_a.iter().map(|&idx| {
                                    let p = mesh_a.vertices[idx];
                                    let diff = p - c_a;
                                    Point2D { x: diff.dot(u), y: diff.dot(v) }
                                }).collect();

                                let poly_b_2d: Vec<Point2D> = f_b.iter().map(|&idx| {
                                    let p = mesh_b.vertices[idx];
                                    let diff = p - c_a;
                                    Point2D { x: diff.dot(u), y: diff.dot(v) }
                                }).collect();

                                let clipped = clip_polygon(&poly_b_2d, &poly_a_2d);
                                if clipped.len() >= 3 {
                                    let (area, centroid_2d) = polygon_area_and_centroid(&clipped);
                                    if area > 1e-6 {
                                        let centroid_3d = c_a + u * centroid_2d.x + v * centroid_2d.y;
                                        let mut vertices_3d = Vec::with_capacity(clipped.len());
                                        for p_2d in &clipped {
                                            let p_3d = c_a + u * p_2d.x + v * p_2d.y;
                                            vertices_3d.push([p_3d.x, p_3d.y, p_3d.z]);
                                        }

                                        let mut guard = contact_interfaces.lock().unwrap();
                                        guard.push(ContactInterface {
                                            block_a: mesh_a.name.clone(),
                                            block_b: mesh_b.name.clone(),
                                            area_m2: area,
                                            centroid: [centroid_3d.x, centroid_3d.y, centroid_3d.z],
                                            normal: [n_a.x, n_a.y, n_a.z],
                                            vertices_3d,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    });

    let results = contact_interfaces.into_inner().unwrap();
    serde_json::to_string(&results)
        .map_err(|err| PyValueError::new_err(format!("Failed to serialize assembly contacts: {}", err)))
}

#[pyfunction]
fn fix_mesh_json(json_str: &str) -> PyResult<String> {
    let mut bytes = json_str.as_bytes().to_vec();
    let mut obj: CompasDataObject = simd_json::serde::from_slice(&mut bytes)
        .map_err(|err| PyValueError::new_err(format!("Malformed COMPAS JSON Schema: {}", err)))?;

    let (vertices, faces) = obj.data.get_vertices_and_faces();

    let (welded_vertices, welded_faces, weld_details) = weld_vertices(&vertices, &faces);
    let (fixed_faces, flip_details) = unify_winding_directions(welded_faces);

    let welded_count = weld_details.len();
    let flipped_count = flip_details.len();

    obj.data.vertices = Some(welded_vertices);
    obj.data.faces = Some(fixed_faces);
    obj.data.vertex = None;
    obj.data.face = None;

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
fn fix_mesh_buffers(
    py: Python<'_>,
    vertices_obj: &Bound<'_, PyAny>,
    face_indices_obj: &Bound<'_, PyAny>,
    face_offsets_obj: &Bound<'_, PyAny>,
) -> PyResult<String> {
    let v_buf = PyBuffer::<f64>::get(vertices_obj).map_err(|e| PyValueError::new_err(format!("v buffer: {}", e)))?;
    let idx_buf = PyBuffer::<i32>::get(face_indices_obj).map_err(|e| PyValueError::new_err(format!("idx buffer: {}", e)))?;
    let off_buf = PyBuffer::<i32>::get(face_offsets_obj).map_err(|e| PyValueError::new_err(format!("off buffer: {}", e)))?;

    let v_slice = v_buf.as_slice(py).ok_or_else(|| PyValueError::new_err("v not contiguous"))?;
    let idx_slice = idx_buf.as_slice(py).ok_or_else(|| PyValueError::new_err("idx not contiguous"))?;
    let off_slice = off_buf.as_slice(py).ok_or_else(|| PyValueError::new_err("off not contiguous"))?;

    let v_flat: &[f64] = unsafe { std::mem::transmute(v_slice) };
    let idx: &[i32] = unsafe { std::mem::transmute(idx_slice) };
    let off: &[i32] = unsafe { std::mem::transmute(off_slice) };

    let vertex_count = v_flat.len() / 3;
    let mut vertices = Vec::with_capacity(vertex_count);
    for chunk in v_flat.chunks_exact(3) {
        vertices.push(vec![chunk[0], chunk[1], chunk[2]]);
    }

    let face_count = if off.is_empty() { 0 } else { off.len() - 1 };
    let mut faces = Vec::with_capacity(face_count);
    for i in 0..face_count {
        let start = off[i] as usize;
        let end = off[i + 1] as usize;
        let mut face = Vec::with_capacity(end - start);
        for &idx_val in &idx[start..end] {
            face.push(idx_val as usize);
        }
        faces.push(face);
    }

    let (welded_vertices, welded_faces, weld_details) = weld_vertices(&vertices, &faces);
    let (fixed_faces, flip_details) = unify_winding_directions(welded_faces);

    let welded_count = weld_details.len();
    let flipped_count = flip_details.len();

    let mut out_vertices = Vec::with_capacity(welded_vertices.len() * 3);
    for v in &welded_vertices {
        out_vertices.push(v[0]);
        out_vertices.push(v[1]);
        out_vertices.push(v[2]);
    }

    let mut out_idx = Vec::new();
    let mut out_off = Vec::with_capacity(fixed_faces.len() + 1);
    out_off.push(0);
    for face in &fixed_faces {
        for &v_idx in face {
            out_idx.push(v_idx as i32);
        }
        out_off.push(out_idx.len() as i32);
    }

    let report = FixedBuffersReport {
        vertices: out_vertices,
        face_indices: out_idx,
        face_offsets: out_off,
        welded_count,
        flipped_count,
        weld_details,
        flip_details,
    };

    serde_json::to_string(&report)
        .map_err(|err| PyValueError::new_err(format!("Failed to serialize fixed buffers report: {}", err)))
}

#[pyfunction]
fn run_preflight_json(json_str: &str, profile: &str) -> PyResult<String> {
    let mut bytes = json_str.as_bytes().to_vec();
    let obj: CompasDataObject = simd_json::serde::from_slice(&mut bytes)
        .map_err(|err| PyValueError::new_err(format!("Malformed COMPAS JSON Schema (SIMD): {}", err)))?;

    let (vertices, faces) = obj.data.get_vertices_and_faces();

    let bbox = AABB::from_vertices(&vertices);
    let volume_m3 = compute_mesh_volume(&vertices, &faces);
    let boundary_edges = find_boundary_edges(&faces);
    let boundary_edges_count = boundary_edges.len();
    let is_watertight = boundary_edges_count == 0;

    let bounds_x_dim = bbox.max_x - bbox.min_x;
    let bounds_y_dim = bbox.max_y - bbox.min_y;
    let bounds_z_dim = bbox.max_z - bbox.min_z;

    let unique_edges_count = count_unique_edges(&faces);
    let euler_characteristic = (vertices.len() as i32) - (unique_edges_count as i32) + (faces.len() as i32);
    
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
    
    let planarity_ok = max_planarity_deviation <= 0.005; 
    let mesh_quality_ok = min_face_quality >= 0.1; 
    
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
fn run_preflight_buffers(
    py: Python<'_>,
    vertices_obj: &Bound<'_, PyAny>,
    face_indices_obj: &Bound<'_, PyAny>,
    face_offsets_obj: &Bound<'_, PyAny>,
    profile: &str,
) -> PyResult<String> {
    let v_buf = PyBuffer::<f64>::get(vertices_obj).map_err(|e| PyValueError::new_err(format!("v buffer: {}", e)))?;
    let idx_buf = PyBuffer::<i32>::get(face_indices_obj).map_err(|e| PyValueError::new_err(format!("idx buffer: {}", e)))?;
    let off_buf = PyBuffer::<i32>::get(face_offsets_obj).map_err(|e| PyValueError::new_err(format!("off buffer: {}", e)))?;

    let v_slice = v_buf.as_slice(py).ok_or_else(|| PyValueError::new_err("v not contiguous"))?;
    let idx_slice = idx_buf.as_slice(py).ok_or_else(|| PyValueError::new_err("idx not contiguous"))?;
    let off_slice = off_buf.as_slice(py).ok_or_else(|| PyValueError::new_err("off not contiguous"))?;

    let v_flat: &[f64] = unsafe { std::mem::transmute(v_slice) };
    let idx: &[i32] = unsafe { std::mem::transmute(idx_slice) };
    let off: &[i32] = unsafe { std::mem::transmute(off_slice) };

    let vertex_count = v_flat.len() / 3;
    let mut vertices = Vec::with_capacity(vertex_count);
    for chunk in v_flat.chunks_exact(3) {
        vertices.push(vec![chunk[0], chunk[1], chunk[2]]);
    }

    let face_count = if off.is_empty() { 0 } else { off.len() - 1 };
    let mut faces = Vec::with_capacity(face_count);
    for i in 0..face_count {
        let start = off[i] as usize;
        let end = off[i + 1] as usize;
        let mut face = Vec::with_capacity(end - start);
        for &idx_val in &idx[start..end] {
            face.push(idx_val as usize);
        }
        faces.push(face);
    }

    let bbox = AABB::from_vertices(&vertices);
    let volume_m3 = compute_mesh_volume(&vertices, &faces);
    let boundary_edges = find_boundary_edges(&faces);
    let boundary_edges_count = boundary_edges.len();
    let is_watertight = boundary_edges_count == 0;

    let bounds_x_dim = bbox.max_x - bbox.min_x;
    let bounds_y_dim = bbox.max_y - bbox.min_y;
    let bounds_z_dim = bbox.max_z - bbox.min_z;

    let unique_edges_count = count_unique_edges(&faces);
    let euler_characteristic = (vertices.len() as i32) - (unique_edges_count as i32) + (faces.len() as i32);
    
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
    
    let planarity_ok = max_planarity_deviation <= 0.005; 
    let mesh_quality_ok = min_face_quality >= 0.1; 
    
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
        .map_err(|err| PyValueError::new_err(format!("Failed to serialize preflight buffers report: {}", err)))
}

#[pyfunction]
fn detect_clashes_json(items: Vec<(String, String)>, clearance_tolerance: f64) -> PyResult<String> {
    let parts: Vec<SpatialPart> = items
        .par_iter()
        .enumerate()
        .filter_map(|(idx, (name, json_str))| {
            if let Ok(obj) = serde_json::from_str::<CompasDataObject>(json_str) {
                let (vertices, faces) = obj.data.get_vertices_and_faces();
                let bbox = AABB::from_vertices(&vertices);
                Some(SpatialPart {
                    id: idx,
                    name: name.clone(),
                    bbox,
                    vertices: vertices.clone(),
                    faces: faces.clone(),
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
    m.add_function(wrap_pyfunction!(validate_mesh_buffers, m)?)?;
    m.add_function(wrap_pyfunction!(check_swept_collision, m)?)?;
    m.add_function(wrap_pyfunction!(compute_assembly_contacts, m)?)?;
    m.add_function(wrap_pyfunction!(fix_mesh_buffers, m)?)?;
    m.add_function(wrap_pyfunction!(run_preflight_buffers, m)?)?;
    
    m.add_function(wrap_pyfunction!(register_mesh, m)?)?;
    m.add_function(wrap_pyfunction!(clear_mesh_registry, m)?)?;
    m.add_function(wrap_pyfunction!(check_swept_collision_cached, m)?)?;
    
    Ok(())
}