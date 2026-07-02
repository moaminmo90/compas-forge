# COMPAS Forge 🛠️

<p align="center">
  <a href="LICENSE">
    <img src="https://img.shields.io/github/license/moaminmo90/compas-forge" alt="License">
  </a>
  <a href="https://www.python.org/">
    <img src="https://img.shields.io/badge/Python-3.14+-blue.svg" alt="Python">
  </a>
  <a href="https://www.rust-lang.org/">
    <img src="https://img.shields.io/badge/Rust-1.96+-orange.svg" alt="Rust">
  </a>
  <a href="https://compas.dev/">
    <img src="https://img.shields.io/badge/Built%20for-COMPAS-purple" alt="COMPAS">
  </a>
  <img src="https://img.shields.io/badge/Status-Research-green" alt="Status">
</p>

> **An Open-Source, High-Performance Rust-Backed Geometry Verification & Preflight Assembly Clearance Engine for the COMPAS Framework**

<p align="center">
  <img src="assets/banner.png" alt="COMPAS Forge Showcase" width="100%"/>
</p>

COMPAS Forge is an open-source, high-performance geometry verification and digital fabrication preflight suite developed to bridge the gap between computational design environments such as Rhino, Grasshopper, and Blender, and real-world physical manufacturing.

Bound to the **COMPAS** ecosystem, this library provides microsecond-precision topological and physical validation checks, optimizing CAD models before exporting them to robotic paths or CNC machinery.

*This is an open-source, research-oriented library designed for academic collaboration. We invite researchers, roboticists, and computational designers to contribute, extend fabrication profiles, and integrate advanced geometric solvers.*

---

## Installation

### Standard Installation

Once released, COMPAS Forge can be installed from PyPI:

```bash
pip install compas-forge
```

### Installation from Source

If you wish to modify the Rust core, ensure that you have the Rust toolchain installed.

Requirements:

- Rustc 1.96+
- Python 3.14+

```bash
git clone https://github.com/moaminmo90/compas-forge.git
cd compas-forge
pip install -e .
```

---

## Python API Usage Guide

You can import `compas_forge` directly inside Python scripts, Grasshopper Python components, or Blender scripts.

---

### 1. High-Fidelity Preflight Verification

Verify whether a model complies with manufacturing constraints such as dimensions, weight, watertightness, and robotic fabrication profile limits.

```python
import compas_forge

# Run preflight against the KUKA robotic timber fabrication profile
report = compas_forge.run_preflight_profile(
    "my_geometry.json",
    "kuka-timber"
)

if report["is_compliant"]:
    print("✔️ Model is safe for robotic toolpaths.")
else:
    print("❌ Preflight violations found!")
    print(f"  • Watertight: {report['is_watertight']}")
    print(f"  • Calculated Mass: {report['estimated_mass_kg']:.3f} kg")
    print(f"  • Open Holes / Naked Edges: {report['boundary_edges_count']}")
```

---

### 2. Auto-Repairing Topological Defects

Automatically weld duplicate vertices and unify face winding directions using a Rust-backed BFS dual-graph traversal.

```python
import json
import compas_forge

# Repair on the fly and fetch a detailed audit trail
repair_report = compas_forge.fix_geometry_file("dirty_mesh.json")

print("Mesh repaired successfully:")
print(f"  • Merged Vertices: {repair_report['welded_count']}")
print(f"  • Corrected Face Windings: {repair_report['flipped_count']}")

# Export the clean COMPAS structure back to JSON
fixed_data = json.loads(repair_report["fixed_json"])

with open("repaired_mesh.json", "w", encoding="utf-8") as f:
    json.dump(fixed_data, f, indent=4)
```

---

### 3. Assembly Collision & Clearance Solver

Find spatial interferences and clearance violations among multiple CAD components using R*-Tree broad-phase filtering and exact narrow-phase distance checks.

```python
import compas_forge

assembly_files = {
    "beam_a.json": open("beam_a.json", encoding="utf-8").read(),
    "beam_b.json": open("beam_b.json", encoding="utf-8").read(),
}

# Find collisions with a strict 5cm safety clearance threshold
violations = compas_forge.check_assembly_clashes(
    assembly_files,
    clearance_tolerance=0.05
)

for idx, violation in enumerate(violations, 1):
    incident_type = (
        "Collision"
        if violation["has_intersection"]
        else "Clearance Violation"
    )

    print(
        f"[{idx}] "
        f"{violation['part_a']} <-> {violation['part_b']} | "
        f"Distance: {violation['minimum_distance']:.5f} m | "
        f"Type: {incident_type}"
    )
```

---

## CLI Usage Guide

COMPAS Forge is also packaged with an auto-documented command line interface built with `click`.

---

### 1. General Help Menu

```bash
python -m compas_forge --help
```

---

### 2. Run Preflight Audit

```bash
python -m compas_forge preflight my_geometry.json \
  --profile kuka-timber \
  -r preflight_report.html
```

---

### 3. Run Assembly Clash Detection

```bash
python -m compas_forge clash mesh_a.json mesh_b.json \
  --clearance 0.05
```

---

### 4. Execute Mesh Repair Auto-Fixer

```bash
python -m compas_forge fix dirty_mesh.json \
  -o repaired_mesh.json
```

---

## Mathematical Formulations & Algorithms

### 1. Mesh Volume via Gauss's Divergence Theorem

The exact volume $V$ of an arbitrary closed manifold mesh is calculated by summing signed tetrahedra formed from the origin to each boundary triangle:

$$
V = \frac{1}{6} \sum_i \mathbf{p}_0 \cdot \left(\mathbf{p}_1 \times \mathbf{p}_2\right)
$$

where $\mathbf{p}_0$, $\mathbf{p}_1$, and $\mathbf{p}_2$ are the vertex coordinates of each triangulated face.

---

### 2. Best-Fit Newell Plane & Planarity Deviation

AEC facade rationalization and timber stock cutting often require robust flatness evaluation. Best-fit reference plane normals are calculated using Newell's method:

$$
n_x = \sum_{i=0}^{N-1} (y_i - y_{i+1})(z_i + z_{i+1})
$$

$$
n_y = \sum_{i=0}^{N-1} (z_i - z_{i+1})(x_i + x_{i+1})
$$

$$
n_z = \sum_{i=0}^{N-1} (x_i - x_{i+1})(y_i + y_{i+1})
$$

The maximum perpendicular distance $d_{max}$ of any vertex $\mathbf{v}_i$ to the centroid plane $\mathbf{c}$ is evaluated as:

$$
d_{max} = \max_i \left|(\mathbf{v}_i - \mathbf{c}) \cdot \mathbf{n}\right|
$$

---

### 3. Topological Invariants

Algebraic topology validation checks the Euler characteristic $\chi$ and genus $g$ of closed manifold meshes to guarantee topological integrity:

$$
\chi = V - E + F
$$

$$
g = \frac{2 - \chi}{2}
$$

where $V$ is the vertex count, $E$ is the unique undirected edge count, and $F$ is the face count.

---

## Author & Academic Profile

Developed by **Mohammad Amin Moradi** at the intersection of computational geometry, systems programming, and digital fabrication in AEC.

- **GitHub:** <https://github.com/moaminmo90>
- **LinkedIn:** <https://www.linkedin.com/in/moaminmo90>

---

## License

Licensed under the **MIT License**.

See the **[LICENSE](LICENSE)** file for details.