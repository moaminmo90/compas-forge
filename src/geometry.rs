use rayon::prelude::*;
use std::collections::{HashMap, HashSet, VecDeque};
use rstar::{RTreeObject, AABB as RStarAABB};

/// Represents an Axis-Aligned Bounding Box (AABB) in 3D space
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AABB {
    pub min_x: f64,
    pub min_y: f64,
    pub min_z: f64,
    pub max_x: f64,
    pub max_y: f64,
    pub max_z: f64,
}

impl AABB {
    pub fn from_vertices(vertices: &[Vec<f64>]) -> Self {
        if vertices.is_empty() {
            return Self { min_x: 0.0, min_y: 0.0, min_z: 0.0, max_x: 0.0, max_y: 0.0, max_z: 0.0 };
        }
        let mut min_x = f64::MAX;
        let mut min_y = f64::MAX;
        let mut min_z = f64::MAX;
        let mut max_x = f64::MIN;
        let mut max_y = f64::MIN;
        let mut max_z = f64::MIN;

        for v in vertices {
            if v.len() >= 3 {
                if v[0] < min_x { min_x = v[0]; }
                if v[0] > max_x { max_x = v[0]; }
                if v[1] < min_y { min_y = v[1]; }
                if v[1] > max_y { max_y = v[1]; }
                if v[2] < min_z { min_z = v[2]; }
                if v[2] > max_z { max_z = v[2]; }
            }
        }

        Self { min_x, min_y, min_z, max_x, max_y, max_z }
    }
}

/// Dynamic spatial part holding geometric topological elements and spatial structures
#[derive(Debug, Clone)]
pub struct SpatialPart {
    pub id: usize,
    pub name: String,
    pub bbox: AABB,
    pub vertices: Vec<Vec<f64>>,
    pub faces: Vec<Vec<usize>>,
}

impl RTreeObject for SpatialPart {
    type Envelope = RStarAABB<[f64; 3]>;

    fn envelope(&self) -> Self::Envelope {
        RStarAABB::from_corners(
            [self.bbox.min_x, self.bbox.min_y, self.bbox.min_z],
            [self.bbox.max_x, self.bbox.max_y, self.bbox.max_z],
        )
    }
}

/// Helper to triangulate any arbitrary n-gon face using a fan triangulation pattern.
pub fn triangulate_face(face: &[usize]) -> Vec<[u32; 3]> {
    let mut triangles = Vec::new();
    if face.len() < 3 {
        return triangles;
    }
    let v0 = face[0] as u32;
    for i in 1..(face.len() - 1) {
        let v1 = face[i] as u32;
        let v2 = face[i + 1] as u32;
        triangles.push([v0, v1, v2]);
    }
    triangles
}

/// Computes the exact signed volume of an arbitrary closed triangle mesh.
pub fn compute_mesh_volume(vertices: &[Vec<f64>], faces: &[Vec<usize>]) -> f64 {
    if vertices.is_empty() || faces.is_empty() {
        return 0.0;
    }
    let total_volume = faces.par_iter().map(|face| {
        let tris = triangulate_face(face);
        let mut local_vol = 0.0;
        for tri in tris {
            let p0 = &vertices[tri[0] as usize];
            let p1 = &vertices[tri[1] as usize];
            let p2 = &vertices[tri[2] as usize];
            if p0.len() >= 3 && p1.len() >= 3 && p2.len() >= 3 {
                let triple_product = p0[0] * (p1[1] * p2[2] - p1[2] * p2[1])
                                   - p0[1] * (p1[0] * p2[2] - p1[2] * p2[0])
                                   + p0[2] * (p1[0] * p2[1] - p1[1] * p2[0]);
                local_vol += triple_product;
            }
        }
        local_vol
    }).sum::<f64>();

    (total_volume / 6.0).abs()
}

/// Counts unique undirected edges in parallel. Used for Euler characteristic calculation.
pub fn count_unique_edges(faces: &[Vec<usize>]) -> usize {
    if faces.is_empty() {
        return 0;
    }
    let unique_edges: HashSet<(usize, usize)> = faces
        .par_iter()
        .flat_map(|face| {
            let mut edges = Vec::new();
            let len = face.len();
            if len < 3 { return edges; }
            for i in 0..len {
                let u = face[i];
                let v = face[(i + 1) % len];
                let edge = if u < v { (u, v) } else { (v, u) };
                edges.push(edge);
            }
            edges
        })
        .collect();
    unique_edges.len()
}

/// Computes maximum planarity deviation of faces using Newell's Method.
pub fn compute_max_planarity_deviation(vertices: &[Vec<f64>], faces: &[Vec<usize>]) -> f64 {
    if vertices.is_empty() || faces.is_empty() {
        return 0.0;
    }
    faces.par_iter().map(|face| {
        let len = face.len();
        if len < 4 {
            return 0.0; // Triangles are always perfectly planar
        }
        
        // 1. Centroid
        let mut cx = 0.0;
        let mut cy = 0.0;
        let mut cz = 0.0;
        for &idx in face {
            let v = &vertices[idx];
            cx += v[0];
            cy += v[1];
            cz += v[2];
        }
        cx /= len as f64;
        cy /= len as f64;
        cz /= len as f64;

        // 2. Newell's Normal Fitting
        let mut nx = 0.0;
        let mut ny = 0.0;
        let mut nz = 0.0;
        for i in 0..len {
            let idx_i = face[i];
            let idx_j = face[(i + 1) % len];
            let vi = &vertices[idx_i];
            let vj = &vertices[idx_j];
            nx += (vi[1] - vj[1]) * (vi[2] + vj[2]);
            ny += (vi[2] - vj[2]) * (vi[0] + vj[0]);
            nz += (vi[0] - vj[0]) * (vi[1] + vj[1]);
        }
        let norm = (nx*nx + ny*ny + nz*nz).sqrt();
        if norm < 1e-12 {
            return 0.0;
        }
        nx /= norm;
        ny /= norm;
        nz /= norm;

        // 3. Maximum distance of any face vertex to the Newell plane
        let mut max_dev = 0.0;
        for &idx in face {
            let v = &vertices[idx];
            let dx = v[0] - cx;
            let dy = v[1] - cy;
            let dz = v[2] - cz;
            let dist = (dx*nx + dy*ny + dz*nz).abs();
            if dist > max_dev {
                max_dev = dist;
            }
        }
        max_dev
    })
    .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
    .unwrap_or(0.0)
}

/// Evaluates normalized triangle geometric quality metric (q) in parallel.
/// Equitable triangles yield q = 1.0, degenerate flat triangles yield q = 0.0.
pub fn compute_min_face_quality(vertices: &[Vec<f64>], faces: &[Vec<usize>]) -> f64 {
    if vertices.is_empty() || faces.is_empty() {
        return 1.0;
    }
    faces.par_iter().map(|face| {
        let tris = triangulate_face(face);
        let mut min_tri_q = 1.0;
        for tri in tris {
            let p0 = &vertices[tri[0] as usize];
            let p1 = &vertices[tri[1] as usize];
            let p2 = &vertices[tri[2] as usize];
            
            let a = ((p0[0]-p1[0]).powi(2) + (p0[1]-p1[1]).powi(2) + (p0[2]-p1[2]).powi(2)).sqrt();
            let b = ((p1[0]-p2[0]).powi(2) + (p1[1]-p2[1]).powi(2) + (p1[2]-p2[2]).powi(2)).sqrt();
            let c = ((p2[0]-p0[0]).powi(2) + (p2[1]-p0[1]).powi(2) + (p2[2]-p0[2]).powi(2)).sqrt();
            
            if a < 1e-12 || b < 1e-12 || c < 1e-12 {
                min_tri_q = 0.0;
                continue;
            }

            // Heron's formula for triangle area
            let s = (a + b + c) / 2.0;
            let area_sq = s * (s - a) * (s - b) * (s - c);
            let area = if area_sq > 0.0 { area_sq.sqrt() } else { 0.0 };

            let q = (4.0 * 3.0f64.sqrt() * area) / (a*a + b*b + c*c);
            if q < min_tri_q {
                min_tri_q = q;
            }
        }
        min_tri_q
    })
    .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
    .unwrap_or(1.0)
}

/// Extracts boundary (naked) edges representing exact open topological bounds (holes).
pub fn find_boundary_edges(faces: &[Vec<usize>]) -> Vec<(usize, usize)> {
    if faces.is_empty() {
        return Vec::new();
    }

    let mut edge_occurrences = HashMap::new();
    for face in faces {
        let len = face.len();
        if len < 3 { continue; }
        for i in 0..len {
            let u = face[i];
            let v = face[(i + 1) % len];
            let edge = if u < v { (u, v) } else { (v, u) };
            *edge_occurrences.entry(edge).or_insert(0) += 1;
        }
    }

    edge_occurrences
        .into_iter()
        .filter(|&(_, count)| count == 1)
        .map(|(edge, _)| edge)
        .collect()
}

/// High-performance parallel detection of non-manifold topology edges
pub fn find_non_manifold_edges(faces: &[Vec<usize>]) -> Vec<(usize, usize)> {
    if faces.is_empty() {
        return Vec::new();
    }

    let edge_occurrences: HashMap<(usize, usize), usize> = faces
        .par_iter()
        .flat_map(|face| {
            let mut edges = Vec::new();
            let len = face.len();
            if len < 3 {
                return edges;
            }
            for i in 0..len {
                let u = face[i];
                let v = face[(i + 1) % len];
                let edge = if u < v { (u, v) } else { (v, u) };
                edges.push(edge);
            }
            edges
        })
        .fold(HashMap::new, |mut acc, edge| {
            *acc.entry(edge).or_insert(0) += 1;
            acc
        })
        .reduce(HashMap::new, |mut map1, map2| {
            for (edge, count) in map2 {
                *map1.entry(edge).or_insert(0) += count;
            }
            map1
        });

    edge_occurrences
        .into_par_iter()
        .filter(|&(_, count)| count > 2)
        .map(|(edge, _)| edge)
        .collect()
}

/// Audit log representing a single welded vertex operation
#[derive(Debug, serde::Serialize, Clone)]
pub struct WeldAudit {
    pub old_index: usize,
    pub merged_into: usize,
    pub coordinates: Vec<f64>,
}

/// SOTA Vertex Welding algorithm with exact topological lineage tracking.
pub fn weld_vertices(vertices: &[Vec<f64>], faces: &[Vec<usize>]) -> (Vec<Vec<f64>>, Vec<Vec<usize>>, Vec<WeldAudit>) {
    if vertices.is_empty() {
        return (Vec::new(), Vec::new(), Vec::new());
    }
    
    let mut unique_vertices = Vec::new();
    let mut index_map = HashMap::new();
    let mut old_to_new = vec![0; vertices.len()];
    let mut weld_audit_logs = Vec::new();
    
    for (old_idx, v) in vertices.iter().enumerate() {
        if v.len() < 3 { continue; }
        let sig = format!("{:.6},{:.6},{:.6}", v[0], v[1], v[2]);
        match index_map.entry(sig) {
            std::collections::hash_map::Entry::Occupied(entry) => {
                let merged_idx = *entry.get();
                old_to_new[old_idx] = merged_idx;
                weld_audit_logs.push(WeldAudit {
                    old_index: old_idx,
                    merged_into: merged_idx,
                    coordinates: v.clone(),
                });
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                let new_idx = unique_vertices.len();
                unique_vertices.push(v.clone());
                entry.insert(new_idx);
                old_to_new[old_idx] = new_idx;
            }
        }
    }
    
    let new_faces: Vec<Vec<usize>> = faces
        .iter()
        .map(|face| {
            face.iter().map(|&old_idx| old_to_new[old_idx]).collect()
        })
        .collect();
        
    (unique_vertices, new_faces, weld_audit_logs)
}

/// Audit log representing a single face flipped operation
#[derive(Debug, serde::Serialize, Clone)]
pub struct FlipAudit {
    pub face_index: usize,
    pub old_winding: Vec<usize>,
    pub new_winding: Vec<usize>,
}

/// SOTA Dual-Graph BFS Normal Winding Unifier with exact geometric audit logging.
pub fn unify_winding_directions(mut faces: Vec<Vec<usize>>) -> (Vec<Vec<usize>>, Vec<FlipAudit>) {
    if faces.is_empty() {
        return (faces, Vec::new());
    }

    let num_faces = faces.len();
    let mut edge_to_faces = HashMap::new();
    for (f_idx, face) in faces.iter().enumerate() {
        let len = face.len();
        if len < 3 { continue; }
        for i in 0..len {
            let u = face[i];
            let v = face[(i + 1) % len];
            let edge = if u < v { (u, v) } else { (v, u) };
            edge_to_faces.entry(edge).or_insert_with(Vec::new).push(f_idx);
        }
    }

    let mut visited = vec![false; num_faces];
    let mut queue = VecDeque::new();
    let mut flip_audit_logs = Vec::new();

    for start_face in 0..num_faces {
        if visited[start_face] {
            continue;
        }

        visited[start_face] = true;
        queue.push_back(start_face);

        while let Some(curr_idx) = queue.pop_front() {
            let curr_face = faces[curr_idx].clone();
            let len = curr_face.len();
            if len < 3 { continue; }

            for i in 0..len {
                let u = curr_face[i];
                let v = curr_face[(i + 1) % len];
                let undirected_edge = if u < v { (u, v) } else { (v, u) };

                if let Some(neighbors) = edge_to_faces.get(&undirected_edge) {
                    for &neigh_idx in neighbors {
                        if visited[neigh_idx] {
                            continue;
                        }

                        let neigh_face = &faces[neigh_idx];
                        let n_len = neigh_face.len();
                        if n_len < 3 { continue; }

                        let mut edge_found = false;
                        let mut same_direction = false;

                        for j in 0..n_len {
                            let nu = neigh_face[j];
                            let nv = neigh_face[(j + 1) % n_len];
                            if (nu == u && nv == v) || (nu == v && nv == u) {
                                edge_found = true;
                                if nu == u && nv == v {
                                    same_direction = true;
                                }
                                break;
                            }
                        }

                        if edge_found {
                            if same_direction {
                                let old_winding = faces[neigh_idx].clone();
                                faces[neigh_idx].reverse();
                                let new_winding = faces[neigh_idx].clone();
                                
                                flip_audit_logs.push(FlipAudit {
                                    face_index: neigh_idx,
                                    old_winding,
                                    new_winding,
                                });
                            }
                            visited[neigh_idx] = true;
                            queue.push_back(neigh_idx);
                        }
                    }
                }
            }
        }
    }

    (faces, flip_audit_logs)
}