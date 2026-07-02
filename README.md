# COMPAS Forge 🛠️

> **An Open-Source, Research-Oriented Geometry Verification & Fabrication Preflight Engine for the COMPAS Ecosystem**

<p align="center">
  <img src="assets/banner.png" alt="COMPAS Forge Banner" width="100%">
</p>

<p align="center">

![License](https://img.shields.io/github/license/moaminmo90/compas-forge)
![Python](https://img.shields.io/badge/Python-3.14+-blue.svg)
![Rust](https://img.shields.io/badge/Rust-1.96+-orange.svg)
![COMPAS](https://img.shields.io/badge/Built%20for-COMPAS-purple)
![Status](https://img.shields.io/badge/Status-Research-green)

</p>

---

# Overview

COMPAS Forge is an open-source, high-performance geometry verification and fabrication preflight engine developed for the **COMPAS** ecosystem.

It bridges computational geometry with digital fabrication by performing fast topology validation, manufacturability checks, collision analysis, and automatic mesh repair before fabrication.

The core engine is written in **Rust** and exposed to Python via **PyO3** and **Maturin**, combining Python usability with systems-level performance.

---

# Features

- SIMD-Accelerated JSON Parsing (`simd-json`)
- Parallel Topology Analysis (`Rayon`)
- Automatic Mesh Repair
- Spatial Clash Detection
- Manufacturing Rule Verification
- Interactive HTML Reports
- WebGL 3D Viewer (Three.js)
- Industrial Fabrication Profiles
- Rust + Python Architecture
- Research-Oriented Design

---

# Showcase

<p align="center">
<img src="assets/banner.png" width="100%">
</p>

---

# Research Vision

Modern digital fabrication workflows require reliable geometric verification before robotic execution.

COMPAS Forge provides a non-destructive verification pipeline capable of validating:

- Topology
- Watertightness
- Mesh Quality
- Spatial Clearance
- Manufacturing Constraints
- Fabrication Profiles

before geometry reaches robotic systems such as:

- COMPAS Timber
- COMPAS FAB
- PyBullet
- MoveIt
- CNC Toolchains

---

# Technology Stack

| Component | Technology |
|------------|------------|
| Language | Rust |
| Python Binding | PyO3 |
| Build System | Maturin |
| JSON Parser | simd-json |
| Parallelism | Rayon |
| Spatial Index | rstar |
| Collision Solver | parry3d-f64 |
| Web Viewer | Three.js |
| HTML Dashboard | TailwindCSS |

---

# Mathematical Foundations

## Mesh Volume

Signed mesh volume is computed using Gauss' Divergence Theorem.

\[
V=\frac16\sum_i p_0\cdot(p_1\times p_2)
\]

---

## Newell Plane Estimation

Polygon planarity is evaluated using Newell's Method.

Maximum deviation:

\[
d_{max}=\max_i |(v_i-c)\cdot n|
\]

---

## Euler Characteristic

\[
\chi = V-E+F
\]

Genus

\[
g=\frac{2-\chi}{2}
\]

---

## Facet Quality

\[
q=\frac{4\sqrt3A}{a^2+b^2+c^2}
\]

---

# Installation

## Requirements

- Rust 1.96+
- Python 3.14+

Clone the repository:

```bash
git clone https://github.com/moaminmo90/compas-forge.git

cd compas-forge
```

Install:

```bash
pip install -e .
```

---

# Command Line Interface

Display help

```bash
python -m compas_forge --help
```

---

## Preflight

Run manufacturing verification.

```bash
python -m compas_forge preflight model.json \
    --profile kuka-timber \
    --report report.html
```

Options

| Option | Description |
|---------|-------------|
| --profile | Fabrication profile |
| --report | HTML report output |

---

## Clash Detection

```bash
python -m compas_forge clash mesh_a.json mesh_b.json \
    --clearance 0.05
```

---

## Mesh Repair

```bash
python -m compas_forge fix dirty_mesh.json \
    --output repaired_mesh.json
```

---

# Example Output

The generated HTML report includes

- Manufacturing Summary
- Mesh Statistics
- Interactive WebGL Viewer
- Topological Diagnostics
- Clearance Analysis
- Mesh Repair Report
- Pipeline Performance

---

# Roadmap

- [x] Rust Geometry Core
- [x] Manufacturing Profiles
- [x] Interactive HTML Reports
- [x] Clash Detection
- [x] Mesh Repair

Future work

- COMPAS Timber Integration
- COMPAS FAB Integration
- IFC Support
- STEP Import
- glTF Export
- GPU Acceleration
- WebAssembly Viewer

---

# Contributing

Contributions are welcome.

Feel free to submit issues, feature requests, or pull requests.

---

# Author

**Mohammad Amin Moradi**

GitHub

https://github.com/moaminmo90

LinkedIn

https://linkedin.com/in/moaminmo90

---

# License

This project is licensed under the **MIT License**.

See the **LICENSE** file for details.