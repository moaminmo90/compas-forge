from compas.plugins import plugin
import compas_forge

@plugin(category='datastructures', requires=['compas_forge'])
def is_mesh_manifold(mesh):
    """
    Overrides the default COMPAS pure-Python manifold check 
    with the high-performance Rust-backed FFI validation engine.
    """
    try:
        report = compas_forge.verify_mesh_zero_copy(mesh)
        return len(report.get("non_manifold_edges", [])) == 0
    except Exception:
        return False

@plugin(category='datastructures', requires=['compas_forge'])
def is_mesh_closed(mesh):
    """
    Overrides the default COMPAS pure-Python closed/watertightness check 
    with the high-performance Rust-backed FFI validation engine.
    """
    try:
        report = compas_forge.verify_mesh_zero_copy(mesh)
        return report.get("is_valid", False) and report.get("boundary_edges_count", 1) == 0
    except Exception:
        return False