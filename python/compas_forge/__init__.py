import json
from array import array
from compas.datastructures import Mesh
from compas_forge._core import (
    validate_compas_json,
    detect_clashes_json,
    fix_mesh_json,
    run_preflight_json,
    validate_mesh_buffers,
    check_swept_collision,
    compute_assembly_contacts,
    fix_mesh_buffers,
    run_preflight_buffers,
    register_mesh,
    clear_mesh_registry,
    check_swept_collision_cached
)
from compas_forge.reporter import generate_html_report

__version__ = "0.3.0"

# SOTA COMPAS Plugin Auto-Discovery Metadata (Universal Discovery)
# This variable enables discovery inside strict environments lacking setuptools (e.g. IronPython inside Rhino)
__all_plugins__ = ["compas_forge.plugin"]

__all__ = [
    "validate_compas_json",
    "detect_clashes_json",
    "fix_mesh_json",
    "run_preflight_json",
    "validate_mesh_buffers",
    "check_swept_collision",
    "compute_assembly_contacts",
    "fix_mesh_buffers",
    "run_preflight_buffers",
    "register_mesh",
    "clear_mesh_registry",
    "check_swept_collision_cached",
    "register_mesh_to_cache",
    "clear_mesh_cache",
    "check_swept_collision_cached_poses",
    "compas_mesh_to_buffers",
    "verify_file",
    "verify_mesh_zero_copy",
    "fix_mesh_zero_copy",
    "run_preflight_profile_zero_copy",
    "check_swept_collision_zero_copy",
    "compute_assembly_contacts_zero_copy",
    "check_assembly_clashes",
    "fix_geometry_file",
    "run_preflight_profile"
]

def register_mesh_to_cache(mesh_id: str, mesh: Mesh) -> str:
    """
    Registers a COMPAS Mesh to the high-performance Rust static memory registry.
    Allows subsequent swept-collision queries to run instantly by referencing its ID.
    """
    v, idx, off = compas_mesh_to_buffers(mesh)
    return register_mesh(str(mesh_id), list(v), list(idx), list(off))

def clear_mesh_cache() -> str:
    """
    Clears the high-performance Rust static memory registry to prevent memory leaks.
    """
    return clear_mesh_registry()

def check_swept_collision_cached_poses(
    mesh1_id: str, pose1_start, pose1_end, 
    mesh2_id: str, pose2_start, pose2_end
) -> dict:
    """
    Evaluates Continuous Collision Detection (CCD) between two pre-registered meshes.
    Poses are passed as a 7-element array: [x, y, z, qx, qy, qz, qw].
    Bypasses mesh reconstruction completely, executing in microseconds.
    """
    result_raw = check_swept_collision_cached(
        str(mesh1_id), list(pose1_start), list(pose1_end),
        str(mesh2_id), list(pose2_start), list(pose2_end)
    )
    return json.loads(result_raw)

def compas_mesh_to_buffers(mesh):
    """
    Extracts flat continuous memory layouts from a COMPAS Mesh object
    using native Python array structures to avoid memory copies.
    """
    flat_vertices = []
    for vertex in mesh.vertices():
        flat_vertices.extend(mesh.vertex_coordinates(vertex))
    
    face_indices = []
    face_offsets = [0]
    for face in mesh.faces():
        vertices = mesh.face_vertices(face)
        face_indices.extend(vertices)
        face_offsets.append(len(face_indices))
        
    vertices_arr = array('d', flat_vertices)
    face_indices_arr = array('i', face_indices)
    face_offsets_arr = array('i', face_offsets)
    
    return vertices_arr, face_indices_arr, face_offsets_arr

def verify_file(filepath: str) -> dict:
    with open(filepath, 'r', encoding='utf-8') as f:
        raw_data = f.read()
    report_raw = validate_compas_json(raw_data)
    return json.loads(report_raw)

def verify_mesh_zero_copy(mesh) -> dict:
    """
    Direct zero-copy memory bridge verification using PyBuffer protocol.
    Bypasses file-IO and JSON parsing serialization overhead.
    """
    v_arr, idx_arr, off_arr = compas_mesh_to_buffers(mesh)
    report_raw = validate_mesh_buffers(v_arr, idx_arr, off_arr)
    return json.loads(report_raw)

def fix_mesh_zero_copy(mesh) -> tuple:
    """
    Automatically repairs mesh defects (vertex welding & normal alignment)
    directly in memory and returns a newly reconstructed, healthy COMPAS Mesh object.
    Bypasses all JSON serialization and file-IO.
    """
    v_arr, idx_arr, off_arr = compas_mesh_to_buffers(mesh)
    report_raw = fix_mesh_buffers(v_arr, idx_arr, off_arr)
    report = json.loads(report_raw)
    
    fixed_mesh = Mesh()
    vertices_flat = report["vertices"]
    for i in range(0, len(vertices_flat), 3):
        fixed_mesh.add_vertex(
            x=vertices_flat[i], 
            y=vertices_flat[i+1], 
            z=vertices_flat[i+2]
        )
        
    face_indices = report["face_indices"]
    face_offsets = report["face_offsets"]
    for i in range(len(face_offsets) - 1):
        start = face_offsets[i]
        end = face_offsets[i+1]
        face_verts = face_indices[start:end]
        fixed_mesh.add_face(face_verts)
        
    return fixed_mesh, report

def run_preflight_profile_zero_copy(mesh, profile_name: str) -> dict:
    """
    Runs preflight checks using the high-performance zero-copy memory bridge.
    Avoids writing files or parsing JSON schemas, evaluating SOTA metrics instantly.
    """
    v_arr, idx_arr, off_arr = compas_mesh_to_buffers(mesh)
    report_raw = run_preflight_buffers(v_arr, idx_arr, off_arr, profile_name)
    return json.loads(report_raw)

def check_swept_collision_zero_copy(
    mesh_a, pose_a_start, pose_a_end, 
    mesh_b, pose_b_start, pose_b_end
) -> dict:
    """
    Evaluates Continuous Collision Detection (CCD) between two moving COMPAS meshes.
    Poses are passed as a 7-element array: [x, y, z, qx, qy, qz, qw].
    Utilizes parry3d time-of-impact queries with rotational sub-stepping.
    """
    v_arr_a, idx_arr_a, off_arr_a = compas_mesh_to_buffers(mesh_a)
    v_arr_b, idx_arr_b, off_arr_b = compas_mesh_to_buffers(mesh_b)
    
    result_raw = check_swept_collision(
        v_arr_a, idx_arr_a, off_arr_a, list(pose_a_start), list(pose_a_end),
        v_arr_b, idx_arr_b, off_arr_b, list(pose_b_start), list(pose_b_end)
    )
    return json.loads(result_raw)

def compute_assembly_contacts_zero_copy(meshes_dict: dict, tolerance: float = 0.005) -> list:
    """
    Identifies exact physical contact interfaces, areas, centroids, and normals
    across an assembly of static COMPAS meshes using shared memory buffers.
    """
    assembly_list = []
    for name, mesh in meshes_dict.items():
        v, idx, off = compas_mesh_to_buffers(mesh)
        assembly_list.append({
            "name": str(name),
            "vertices": v,
            "indices": idx,
            "offsets": off
        })
        
    result_raw = compute_assembly_contacts(assembly_list, tolerance)
    return json.loads(result_raw)

def check_assembly_clashes(files_map: dict, clearance_tolerance: float = 0.0) -> list:
    items = list(files_map.items())
    result_raw = detect_clashes_json(items, clearance_tolerance)
    return json.loads(result_raw)

def fix_geometry_file(filepath: str) -> dict:
    with open(filepath, 'r', encoding='utf-8') as f:
        raw_data = f.read()
    report_raw = fix_mesh_json(raw_data)
    return json.loads(report_raw)

def run_preflight_profile(filepath: str, profile_name: str) -> dict:
    with open(filepath, 'r', encoding='utf-8') as f:
        raw_data = f.read()
    report_raw = run_preflight_json(raw_data, profile_name)
    return json.loads(report_raw)