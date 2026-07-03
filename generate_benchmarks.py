import time
import math
import os
from compas.datastructures import Mesh
from rich.console import Console
from rich.table import Table
from rich.panel import Panel

# Optional matplotlib import for graceful fallback in systems lacking visual libraries
try:
    import matplotlib
    matplotlib.use('Agg') # Headless backend for terminal execution
    import matplotlib.pyplot as plt
    MATPLOTLIB_AVAILABLE = True
except ImportError:
    MATPLOTLIB_AVAILABLE = False

console = Console()

def generate_benchmark_dome(u_divs, v_divs):
    """
    Generates a dense parametric spherical dome for load testing.
    """
    mesh = Mesh()
    for i in range(u_divs + 1):
        theta = (i / u_divs) * (math.pi / 2.0)
        for j in range(v_divs):
            phi = (j / v_divs) * (2.0 * math.pi)
            x = math.sin(theta) * math.cos(phi)
            y = math.sin(theta) * math.sin(phi)
            z = math.cos(theta)
            mesh.add_vertex(x=x, y=y, z=z)
            
    for i in range(u_divs):
        for j in range(v_divs):
            v0 = i * v_divs + j
            v1 = i * v_divs + ((j + 1) % v_divs)
            v2 = (i + 1) * v_divs + ((j + 1) % v_divs)
            v3 = (i + 1) * v_divs + j
            mesh.add_face([v0, v1, v2, v3])
    return mesh

def run_pure_python_is_closed(mesh):
    """
    Implements a fallback pure-Python is_closed logic to benchmark 
    against COMPAS Forge's Rust implementation without triggering the plugin.
    """
    # Pure Python naked-edge boundary extraction
    edges = {}
    for face in mesh.faces():
        vertices = mesh.face_vertices(face)
        for i in range(len(vertices)):
            u, v = vertices[i], vertices[(i + 1) % len(vertices)]
            edge = tuple(sorted([u, v]))
            edges[edge] = edges.get(edge, 0) + 1
            
    boundary_count = sum(1 for count in edges.values() if count == 1)
    return boundary_count == 0

def execute_benchmarks():
    console.print(Panel.fit(
        "[bold white]COMPAS FORGE INDUSTRIAL SCALING BENCHMARK[/bold white]\n"
        "[dim]Comparing Pure-Python Geometry Queries vs. Rust-Accelerated FFI Buffers[/dim]",
        border_style="cyan"
    ))

    # Test scales: (u_divs, v_divs) -> roughly divs * divs faces
    test_scales = [
        (20, 20),   # ~400 faces
        (40, 40),   # ~1600 faces
        (60, 60),   # ~3600 faces
        (80, 80),   # ~6400 faces
        (100, 100), # ~10000 faces
        (150, 150), # ~22500 faces
        (200, 200)  # ~40000 faces
    ]

    results_python = []
    results_rust = []
    face_counts = []

    table = Table(show_header=True, header_style="bold magenta", border_style="dim")
    table.add_column("Faces Count", style="cyan")
    table.add_column("Pure Python (ms)", style="yellow")
    table.add_column("Rust Forge FFI (ms)", style="green")
    table.add_column("Speedup Factor", style="bold white")

    # Ensure compas_forge plugin is registered and imported
    import compas_forge 

    for u, v in test_scales:
        mesh = generate_benchmark_dome(u, v)
        faces_count = mesh.number_of_faces()
        face_counts.append(faces_count)

        # 1. Benchmark Pure Python
        t0 = time.perf_counter_ns()
        p_res = run_pure_python_is_closed(mesh)
        t_py = (time.perf_counter_ns() - t0) / 1_000_000.0
        results_python.append(t_py)

        # 2. Benchmark Rust-Accelerated Plugin
        t1 = time.perf_counter_ns()
        r_res = mesh.is_closed() # Routed transparently to Rust via COMPAS plugin
        t_rs = (time.perf_counter_ns() - t1) / 1_000_000.0
        results_rust.append(t_rs)

        speedup = t_py / t_rs if t_rs > 0 else 0
        table.add_row(
            f"{faces_count}",
            f"{t_py:.3f} ms",
            f"{t_rs:.3f} ms",
            f"{speedup:.2f}x"
        )

    console.print(table)

    # 4. Generate professional chart using matplotlib if available
    if MATPLOTLIB_AVAILABLE:
        print("\nPlotting performance curves...")
        plt.figure(figsize=(10, 6))
        plt.plot(face_counts, results_python, marker='o', color='#f59e0b', label='Pure Python (COMPAS default)', linewidth=2)
        plt.plot(face_counts, results_rust, marker='s', color='#10b981', label='Rust Accelerated (COMPAS Forge)', linewidth=2)
        
        plt.title('COMPAS Geometry Scaling Performance Benchmark', fontsize=14, fontweight='bold', pad=15)
        plt.xlabel('Mesh Facets (Count)', fontsize=11)
        plt.ylabel('Execution Latency (Milliseconds)', fontsize=11)
        plt.grid(True, linestyle='--', alpha=0.5)
        plt.legend(fontsize=11)
        
        # Save plot to assets directory
        os.makedirs("assets", exist_ok=True)
        chart_path = os.path.join("assets", "benchmark_speedup.png")
        plt.savefig(chart_path, dpi=300, bbox_inches='tight')
        console.print(f"[bold green]✔ Performance benchmarking chart compiled successfully at:[/bold green] {chart_path}")
    else:
        console.print("\n[yellow]⚠ Matplotlib is not installed in the active environment. Skipping plotting phase.[/yellow]")

if __name__ == "__main__":
    execute_benchmarks()