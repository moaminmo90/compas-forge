import time
import math
from compas.datastructures import Mesh
import compas_forge

def generate_voussoir_arch_assembly(num_blocks=30):
    """
    Synthesizes a voussoir arch consisting of wedge blocks.
    Each voussoir is represented as a distinct 3D COMPAS mesh.
    """
    blocks = {}
    r_in, r_out, thickness = 4.0, 4.5, 1.0
    for i in range(num_blocks):
        mesh = Mesh()
        theta0 = (i / num_blocks) * math.pi
        theta1 = ((i + 1) / num_blocks) * math.pi
        
        v_coords = [
            [r_in * math.cos(theta0), -thickness / 2.0, r_in * math.sin(theta0)],
            [r_out * math.cos(theta0), -thickness / 2.0, r_out * math.sin(theta0)],
            [r_out * math.cos(theta1), -thickness / 2.0, r_out * math.sin(theta1)],
            [r_in * math.cos(theta1), -thickness / 2.0, r_in * math.sin(theta1)],
            [r_in * math.cos(theta0), thickness / 2.0, r_in * math.sin(theta0)],
            [r_out * math.cos(theta0), thickness / 2.0, r_out * math.sin(theta0)],
            [r_out * math.cos(theta1), thickness / 2.0, r_out * math.sin(theta1)],
            [r_in * math.cos(theta1), thickness / 2.0, r_in * math.sin(theta1)]
        ]
        for coord in v_coords:
            mesh.add_vertex(x=coord[0], y=coord[1], z=coord[2])
            
        mesh.add_face([0, 3, 2, 1])
        mesh.add_face([4, 5, 6, 7])
        mesh.add_face([0, 1, 5, 4])
        mesh.add_face([1, 2, 6, 5])
        mesh.add_face([2, 3, 7, 6])
        mesh.add_face([3, 0, 4, 7])
        blocks[f"voussoir_{i}"] = mesh
    return blocks

def run_example():
    num_blocks = 30
    print("=" * 60)
    print(f"  COMPAS FORGE - MASONRY ASSEMBLY CONTACT SOLVER EXAMPLE")
    print("=" * 60)
    
    assembly = generate_voussoir_arch_assembly(num_blocks=num_blocks)
    print(f"Voussoir arch generated with {num_blocks} discrete blocks.")

    # Execute the parallel Sutherland-Hodgman contact manifold solver
    t0 = time.perf_counter_ns()
    contacts = compas_forge.compute_assembly_contacts_zero_copy(assembly, tolerance=0.005)
    latency_ms = (time.perf_counter_ns() - t0) / 1_000_000.0

    print(f"\n[Execution Profiler] Processing Time: {latency_ms:.4f} ms")
    print(f"  Contact Interfaces Identified: {len(contacts)} (Expected: {num_blocks - 1})")
    
    if len(contacts) > 0:
        c0 = contacts[0]
        expected_area = (4.5 - 4.0) * 1.0
        print(f"\nSample Interface Verification (voussoir_0 <-> voussoir_1):")
        print(f"  Touching Parts: {c0['block_a']} <-> {c0['block_b']}")
        print(f"  Clipped Contact Area: {c0['area_m2']:.6f} m² (Expected: {expected_area:.6f})")
        print(f"  Contact Centroid: {c0['centroid']}")
        print(f"  Contact Normal: {c0['normal']}")
        print(f"  Polygon Vertices (3D): {len(c0['vertices_3d'])} Vertices")

if __name__ == "__main__":
    run_example()