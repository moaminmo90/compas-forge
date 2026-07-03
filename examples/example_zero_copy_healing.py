import time
import math
from compas.datastructures import Mesh
import compas_forge

def generate_dirty_dome_structure(u_divs=40, v_divs=40):
    """
    Generates a dense parametric hemisphere dome structure with intentionally
    injected topological defects (duplicates and flipped face windings) for testing.
    """
    mesh = Mesh()
    
    # 1. Generate hemisphere vertices
    for i in range(u_divs + 1):
        theta = (i / u_divs) * (math.pi / 2.0)
        for j in range(v_divs):
            phi = (j / v_divs) * (2.0 * math.pi)
            x = math.sin(theta) * math.cos(phi)
            y = math.sin(theta) * math.sin(phi)
            z = math.cos(theta)
            mesh.add_vertex(x=x, y=y, z=z)
            
    # 2. Inject explicit duplicate vertices at the north pole (theta=0)
    for _ in range(10):
        mesh.add_vertex(x=0.0, y=0.0, z=1.0)
            
    # 3. Construct faces with alternating winding directions
    for i in range(u_divs):
        for j in range(v_divs):
            v0 = i * v_divs + j
            v1 = i * v_divs + ((j + 1) % v_divs)
            v2 = (i + 1) * v_divs + ((j + 1) % v_divs)
            v3 = (i + 1) * v_divs + j
            
            if (i + j) % 2 == 0:
                mesh.add_face([v3, v2, v1, v0]) # Flipped face
            else:
                mesh.add_face([v0, v1, v2, v3]) # Correct face
                
    return mesh

def run_example():
    print("=" * 60)
    print("  COMPAS FORGE - ZERO-COPY IN-MEMORY MESH HEALING EXAMPLE")
    print("=" * 60)
    
    # Generate dirty geometric dome mesh
    dirty_mesh = generate_dirty_dome_structure(u_divs=50, v_divs=50)
    print(f"Dirty dome mesh generated: {dirty_mesh.number_of_vertices()} vertices.")

    # Execute SOTA zero-copy in-memory repair pipeline
    t0 = time.perf_counter_ns()
    fixed_mesh, report = compas_forge.fix_mesh_zero_copy(dirty_mesh)
    latency_ms = (time.perf_counter_ns() - t0) / 1_000_000.0

    print(f"\n[Execution Profiler] Processing Time: {latency_ms:.4f} ms")
    print(f"  Merged Duplicate Vertices: {report['welded_count']}")
    print(f"  Corrected Winding Directions (Flips): {report['flipped_count']}")
    print(f"  Reconstructed Manifold Mesh Vertices: {fixed_mesh.number_of_vertices()}")
    print(f"  Reconstructed Manifold Mesh Faces: {fixed_mesh.number_of_faces()}")
    
    # Verify that the reconstructed mesh is topologically manifold
    print(f"  Is Manifold (Verified by Rust): {fixed_mesh.is_manifold()}")
    print(f"  Is Closed / Watertight (Verified by Rust): {fixed_mesh.is_closed()}")

if __name__ == "__main__":
    run_example()