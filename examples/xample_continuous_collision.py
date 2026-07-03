import time
from compas.datastructures import Mesh
import compas_forge

def create_cube_geometry(size=1.0):
    """
    Generates a 3D manifold cube for swept-collision verification.
    """
    mesh = Mesh()
    h = size / 2.0
    v_coords = [
        [-h, -h, -h], [h, -h, -h], [h, h, -h], [-h, h, -h],
        [-h, -h, h], [h, -h, h], [h, h, h], [-h, h, h]
    ]
    for coord in v_coords:
        mesh.add_vertex(x=coord[0], y=coord[1], z=coord[2])
    mesh.add_face([0, 3, 2, 1])
    mesh.add_face([4, 5, 6, 7])
    mesh.add_face([0, 1, 5, 4])
    mesh.add_face([1, 2, 6, 5])
    mesh.add_face([2, 3, 7, 6])
    mesh.add_face([3, 0, 4, 7])
    return mesh

def run_example():
    print("=" * 60)
    print("  COMPAS FORGE - ROTATIONAL SWEPT-COLLISION (CCD) EXAMPLE")
    print("=" * 60)
    
    cube_a = create_cube_geometry(1.0)
    cube_b = create_cube_geometry(1.0)

    # Pose: [x, y, z, qx, qy, qz, qw]
    # Cube A: Moves from x = -3.0 to x = 3.0, simultaneously rotating 90 degrees around Z axis
    pose_a_start = [-3.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0]
    pose_a_end = [3.0, 0.0, 0.0, 0.0, 0.0, 0.707107, 0.707107]

    # Cube B: Static at origin
    pose_b_start = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0]
    pose_b_end = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0]

    t0 = time.perf_counter_ns()
    result = compas_forge.check_swept_collision_zero_copy(
        cube_a, pose_a_start, pose_a_end,
        cube_b, pose_b_start, pose_b_end
    )
    latency_ms = (time.perf_counter_ns() - t0) / 1_000_000.0

    print(f"\n[Execution Profiler] Evaluation Time: {latency_ms:.4f} ms")
    print(f"  Continuous Collision Detected: {result['has_collision']}")
    print(f"  Exact Time of Impact (TOI): {result['time_of_impact']:.6f} s")
    print(f"  Impact Contact Normal: {result['normal_a']}")
    print(f"  Contact Point A (Witness): {result['witness_a']}")
    print(f"  Contact Point B (Witness): {result['witness_b']}")

if __name__ == "__main__":
    run_example()