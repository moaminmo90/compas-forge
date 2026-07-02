import os
import json

def generate_html_report(diagnostics: dict, preflight: dict, repairs: dict, timing: dict, filepath: str):
    """
    Generates an advanced interactive HTML dashboard for the COMPAS Forge report.
    Uses a highly robust template replacement engine to prevent f-string brace conflicts.
    """
    preflight_status = "COMPLIANT" if preflight.get("is_compliant") else "VIOLATION"
    preflight_color = "green" if preflight.get("is_compliant") else "red"
    
    # Corrected: Restored the geometric extraction lines at the top of the function
    vertices_data = preflight.get("vertices", [])
    faces_data = preflight.get("triangulated_faces", [])
    boundary_edges_data = preflight.get("boundary_edges", [])

    # Generate the detailed lists of repairs for the HTML UI
    weld_rows = ""
    for log in repairs.get("weld_details", []):
        weld_rows += f"""
        <tr class="border-b border-gray-100 hover:bg-gray-50">
            <td class="px-6 py-3 text-sm text-red-600 font-mono">{log['old_index']}</td>
            <td class="px-6 py-3 text-sm text-green-600 font-mono">{log['merged_into']}</td>
            <td class="px-6 py-3 text-sm text-gray-600 font-mono">[{log['coordinates'][0]:.4f}, {log['coordinates'][1]:.4f}, {log['coordinates'][2]:.4f}]</td>
        </tr>
        """
    if not weld_rows:
        weld_rows = '<tr><td colspan="3" class="px-6 py-4 text-center text-sm text-gray-400 italic">No duplicate vertices detected.</td></tr>'

    flip_rows = ""
    for log in repairs.get("flip_details", []):
        flip_rows += f"""
        <tr class="border-b border-gray-100 hover:bg-gray-50">
            <td class="px-6 py-3 text-sm text-gray-800 font-mono">{log['face_index']}</td>
            <td class="px-6 py-3 text-sm text-red-600 font-mono">{log['old_winding']}</td>
            <td class="px-6 py-3 text-sm text-green-600 font-mono">{log['new_winding']}</td>
        </tr>
        """
    if not flip_rows:
        flip_rows = '<tr><td colspan="3" class="px-6 py-4 text-center text-sm text-gray-400 italic">No face winding alignments required.</td></tr>'

    # Build the Performance Profiler Timeline (Waterfall Chart) representation
    total_time = sum(timing.values())
    timeline_html = ""
    for step, ms in timing.items():
        percentage = (ms / total_time * 100) if total_time > 0 else 0
        timeline_html += f"""
        <div class="space-y-1">
            <div class="flex justify-between text-xs font-semibold text-gray-600">
                <span>{step.replace('_', ' ').title()}</span>
                <span class="font-mono text-indigo-600">{ms:.3f} ms ({percentage:.1f}%)</span>
            </div>
            <div class="w-full bg-gray-100 h-3 rounded-full overflow-hidden">
                <div class="bg-indigo-600 h-full rounded-full" style="width: {percentage:.2f}%"></div>
            </div>
        </div>
        """

    # Pure HTML template containing standard JavaScript curly braces
    html_template = """<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>COMPAS Forge Diagnostic Report</title>
    <script src="https://cdn.tailwindcss.com"></script>
    <link href="https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@400;700&family=Inter:wght@400;600;800&display=swap" rel="stylesheet">
    <script src="https://cdnjs.cloudflare.com/ajax/libs/three.js/r128/three.min.js"></script>
    <script src="https://cdn.jsdelivr.net/npm/three@0.128.0/examples/js/controls/OrbitControls.js"></script>
    <style>
        body { font-family: 'Inter', sans-serif; }
        .font-mono { font-family: 'JetBrains Mono', monospace; }
    </style>
</head>
<body class="bg-gray-50 text-gray-900 min-h-screen">
    <header class="bg-white border-b border-gray-200 py-6 px-8 sticky top-0 z-50 shadow-sm">
        <div class="max-w-7xl mx-auto flex justify-between items-center">
            <div>
                <h1 class="text-2xl font-extrabold tracking-tight text-gray-900">COMPAS <span class="text-indigo-600">FORGE</span></h1>
                <p class="text-xs text-gray-500 mt-1">SOTA Manufacturing Preflight & Geometric Verification Audit</p>
            </div>
            <div class="flex items-center space-x-3">
                <span class="text-xs font-semibold text-gray-400 uppercase tracking-wider">Preflight Status:</span>
                <span id="preflight-status-badge" class="px-4 py-1.5 rounded-full text-xs font-bold">
                    __PREFLIGHT_STATUS__
                </span>
            </div>
        </div>
    </header>

    <main class="max-w-7xl mx-auto px-8 py-10 space-y-8">
        <!-- TOP SUMMARY PANELS -->
        <section class="grid grid-cols-1 md:grid-cols-4 gap-6">
            <div class="bg-white p-6 rounded-lg border border-gray-200 shadow-sm">
                <p class="text-xs font-bold text-gray-400 uppercase tracking-wide">Solid Mesh Volume</p>
                <p class="text-2xl font-black text-indigo-600 mt-2 font-mono">__VOLUME_M3__ <span class="text-sm font-normal text-gray-500">m³</span></p>
            </div>
            <div class="bg-white p-6 rounded-lg border border-gray-200 shadow-sm">
                <p class="text-xs font-bold text-gray-400 uppercase tracking-wide">Estimated Mass</p>
                <p class="text-2xl font-black text-indigo-600 mt-2 font-mono">__ESTIMATED_MASS__ <span class="text-sm font-normal text-gray-500">kg</span></p>
            </div>
            <div class="bg-white p-6 rounded-lg border border-gray-200 shadow-sm">
                <p class="text-xs font-bold text-gray-400 uppercase tracking-wide">Topological Holes</p>
                <p id="topological-holes-panel" class="text-2xl font-black mt-2 font-mono">__BOUNDARY_EDGES_COUNT__ <span class="text-sm font-normal text-gray-500">Naked Edges</span></p>
            </div>
            <div class="bg-white p-6 rounded-lg border border-gray-200 shadow-sm">
                <p class="text-xs font-bold text-gray-400 uppercase tracking-wide">Vertices / Faces</p>
                <p class="text-2xl font-black text-gray-800 mt-2 font-mono">__VERTEX_COUNT__ <span class="text-sm font-normal text-gray-400">/</span> __FACE_COUNT__</p>
            </div>
        </section>

        <!-- PERFORMANCE PROFILER TIMELINE & 3D VIEWER -->
        <section class="grid grid-cols-1 md:grid-cols-3 gap-8">
            <!-- Execution Profiler -->
            <div class="bg-white rounded-lg border border-gray-200 shadow-sm overflow-hidden flex flex-col justify-between">
                <div class="border-b border-gray-200 px-6 py-4 bg-gray-50">
                    <h2 class="text-sm font-bold text-gray-800 uppercase tracking-wider">Execution Pipeline Profiler</h2>
                </div>
                <div class="p-6 space-y-4 flex-grow">
                    __TIMELINE_HTML__
                </div>
                <div class="bg-gray-50 border-t border-gray-100 px-6 py-3 text-xs font-mono text-gray-500 flex justify-between">
                    <span>Total Latency:</span>
                    <span class="text-indigo-600 font-bold">__TOTAL_TIME__ ms</span>
                </div>
            </div>

            <!-- SOTA Interactive 3D CAD WebGL Viewport -->
            <div class="bg-white rounded-lg border border-gray-200 shadow-sm overflow-hidden md:col-span-2 flex flex-col">
                <div class="border-b border-gray-200 px-6 py-4 bg-gray-50 flex justify-between items-center">
                    <h2 class="text-sm font-bold text-gray-800 uppercase tracking-wider">Interactive 3D Mesh Inspection (SOTA WebGL)</h2>
                    <span class="text-xs text-indigo-600 font-semibold">[Orbit: Left Click | Pan: Right Click | Zoom: Scroll]</span>
                </div>
                <div id="viewport-3d" class="w-full h-[320px] bg-slate-900 relative"></div>
            </div>
        </section>

        <!-- SOTA TOPOLOGICAL & FACET QUALITY METRICS -->
        <section class="grid grid-cols-1 md:grid-cols-4 gap-6">
            <div class="bg-white p-6 rounded-lg border border-gray-200 shadow-sm">
                <p class="text-xs font-bold text-gray-400 uppercase tracking-wide">Euler Characteristic (χ)</p>
                <p class="text-2xl font-black text-gray-800 mt-2 font-mono">__EULER_CHARACTERISTIC__</p>
                <p class="text-xs text-gray-400 mt-1">Formula: V - E + F</p>
            </div>
            <div class="bg-white p-6 rounded-lg border border-gray-200 shadow-sm">
                <p class="text-xs font-bold text-gray-400 uppercase tracking-wide">Geometric Genus (g)</p>
                <p class="text-2xl font-black text-gray-800 mt-2 font-mono">__GENUS__</p>
                <p class="text-xs text-gray-400 mt-1">Topological Handles Count</p>
            </div>
            <div class="bg-white p-6 rounded-lg border border-gray-200 shadow-sm">
                <p class="text-xs font-bold text-gray-400 uppercase tracking-wide">Max Planarity Deviation</p>
                <p class="text-2xl font-black text-gray-800 mt-2 font-mono">__MAX_PLANARITY__ <span class="text-sm font-normal text-gray-500">m</span></p>
                <p class="text-xs text-gray-400 mt-1">Newell Plane Tolerance Limit</p>
            </div>
            <div class="bg-white p-6 rounded-lg border border-gray-200 shadow-sm">
                <p class="text-xs font-bold text-gray-400 uppercase tracking-wide">Minimum Facet Quality (q)</p>
                <p class="text-2xl font-black text-gray-800 mt-2 font-mono">__MIN_QUALITY__</p>
                <p class="text-xs text-gray-400 mt-1">1.0: Perfect | 0.0: Degenerate</p>
            </div>
        </section>

        <!-- PREFLIGHT VERDICT BOARD -->
        <section class="bg-white rounded-lg border border-gray-200 shadow-sm overflow-hidden">
            <div class="border-b border-gray-200 px-6 py-4 bg-gray-50">
                <h2 class="text-sm font-bold text-gray-800 uppercase tracking-wider">Manufacturing Threshold Limits Check</h2>
            </div>
            <div class="p-6 grid grid-cols-1 md:grid-cols-3 gap-8">
                <!-- Check 1 -->
                <div class="flex items-start space-x-4">
                    <div class="flex-shrink-0 mt-1" id="check-workspace-icon"></div>
                    <div>
                        <h3 class="font-bold text-sm text-gray-800">Workspace Bounds Reach</h3>
                        <p class="text-xs text-gray-500 mt-1">Envelope Size: __BOUNDS_X__m x __BOUNDS_Y__m x __BOUNDS_Z__m</p>
                    </div>
                </div>
                <!-- Check 2 -->
                <div class="flex items-start space-x-4">
                    <div class="flex-shrink-0 mt-1" id="check-watertight-icon"></div>
                    <div>
                        <h3 class="font-bold text-sm text-gray-800">Topological Watertightness</h3>
                        <p class="text-xs text-gray-500 mt-1" id="check-watertight-desc"></p>
                    </div>
                </div>
                <!-- Check 3 -->
                <div class="flex items-start space-x-4">
                    <div class="flex-shrink-0 mt-1" id="check-compliant-icon"></div>
                    <div>
                        <h3 class="font-bold text-sm text-gray-800">Total Structural Weight Check</h3>
                        <p class="text-xs text-gray-500 mt-1">Fabrication Target Profile: <span class="font-mono bg-gray-100 px-1 py-0.5 rounded text-indigo-700">__PROFILE_NAME__</span></p>
                    </div>
                </div>
            </div>
        </section>

        <!-- REPAIR AUDIT LOGS -->
        <section class="grid grid-cols-1 md:grid-cols-2 gap-8">
            <!-- Weld Log -->
            <div class="bg-white rounded-lg border border-gray-200 shadow-sm overflow-hidden">
                <div class="border-b border-gray-200 px-6 py-4 bg-gray-50">
                    <h2 class="text-sm font-bold text-gray-800 uppercase tracking-wider">Vertex Welding Diagnostics (__WELD_COUNT__ Merges)</h2>
                </div>
                <div class="overflow-x-auto max-h-96">
                    <table class="w-full text-left border-collapse">
                        <thead>
                            <tr class="bg-gray-100 border-b border-gray-200 text-xs font-bold text-gray-500 uppercase">
                                <th class="px-6 py-3">Old Index</th>
                                <th class="px-6 py-3">Merged Into</th>
                                <th class="px-6 py-3">Location [X, Y, Z]</th>
                            </tr>
                        </thead>
                        <tbody>
                            __WELD_ROWS__
                        </tbody>
                    </table>
                </div>
            </div>

            <!-- Normal Log -->
            <div class="bg-white rounded-lg border border-gray-200 shadow-sm overflow-hidden">
                <div class="border-b border-gray-200 px-6 py-4 bg-gray-50">
                    <h2 class="text-sm font-bold text-gray-800 uppercase tracking-wider">Normal Winding Orientations (__FLIP_COUNT__ Flips)</h2>
                </div>
                <div class="overflow-x-auto max-h-96">
                    <table class="w-full text-left border-collapse">
                        <thead>
                            <tr class="bg-gray-100 border-b border-gray-200 text-xs font-bold text-gray-500 uppercase">
                                <th class="px-6 py-3">Face Index</th>
                                <th class="px-6 py-3">Original Winding</th>
                                <th class="px-6 py-3">Corrected Winding Order</th>
                            </tr>
                        </thead>
                        <tbody>
                            __FLIP_ROWS__
                        </tbody>
                    </table>
                </div>
            </div>
        </section>
    </main>

    <!-- Native WebGL Script to draw the exact mesh structure -->
    <script>
        const vertices = __VERTICES_JSON__;
        const indices = __FACES_JSON__;
        const boundaryEdges = __BOUNDARY_EDGES_JSON__;
        const isCompliant = __IS_COMPLIANT_JSON__;

        const container = document.getElementById('viewport-3d');
        const scene = new THREE.Scene();
        scene.background = new THREE.Color(0x0f172a); // Slate-900 background

        // Camera
        const camera = new THREE.PerspectiveCamera(45, container.clientWidth / container.clientHeight, 0.01, 1000);
        
        // Renderer
        const renderer = new THREE.WebGLRenderer({ antialias: true });
        renderer.setSize(container.clientWidth, container.clientHeight);
        renderer.setPixelRatio(window.devicePixelRatio);
        container.appendChild(renderer.domElement);

        // OrbitControls
        const controls = new THREE.OrbitControls(camera, renderer.domElement);
        controls.enableDamping = true;
        controls.dampingFactor = 0.05;

        // Construct exact Float32 and Uint32 WebGL Buffers
        const geometry = new THREE.BufferGeometry();
        const vertexArray = new Float32Array(vertices.flat());
        const indexArray = new Uint32Array(indices.flat());

        geometry.setAttribute('position', new THREE.BufferAttribute(vertexArray, 3));
        geometry.setIndex(new THREE.BufferAttribute(indexArray, 1));
        geometry.computeVertexNormals();

        // Color coding: soft green (0x10b981) if compliant, translucent red (0xf43f5e) if violation
        const meshColor = isCompliant ? 0x10b981 : 0xf43f5e;
        const material = new THREE.MeshStandardMaterial({
            color: meshColor,
            roughness: 0.4,
            metalness: 0.1,
            flatShading: true,
            transparent: true,
            opacity: 0.85,
            side: THREE.DoubleSide
        });

        const mesh = new THREE.Mesh(geometry, material);
        scene.add(mesh);

        // Wireframe helper overlay
        const wireframeGeom = new THREE.WireframeGeometry(geometry);
        const wireframeMat = new THREE.LineBasicMaterial({ color: 0xffffff, opacity: 0.2, transparent: true });
        const wireframe = new THREE.LineSegments(wireframeGeom, wireframeMat);
        mesh.add(wireframe);

        // Visual Highlight: Render Naked Edges in thick glowing Red lines
        if (boundaryEdges.length > 0) {
            const lineVertices = [];
            for (const edge of boundaryEdges) {
                const u = vertices[edge[0]];
                const v = vertices[edge[1]];
                lineVertices.push(u[0], u[1], u[2]);
                lineVertices.push(v[0], v[1], v[2]);
            }
            const lineGeom = new THREE.BufferGeometry();
            lineGeom.setAttribute('position', new THREE.Float32BufferAttribute(lineVertices, 3));
            const lineMat = new THREE.LineBasicMaterial({ color: 0xef4444, linewidth: 3 }); // Solid Red line
            const lines = new THREE.LineSegments(lineGeom, lineMat);
            scene.add(lines);
        }

        // Lights
        const ambientLight = new THREE.AmbientLight(0xffffff, 0.4);
        scene.add(ambientLight);

        const dirLight1 = new THREE.DirectionalLight(0xffffff, 0.8);
        dirLight1.position.set(5, 10, 7);
        scene.add(dirLight1);

        const dirLight2 = new THREE.DirectionalLight(0xffffff, 0.3);
        dirLight2.position.set(-5, -5, -5);
        scene.add(dirLight2);

        // Calculate auto-centering bounding logic to avoid GIS floating-point jitter
        geometry.computeBoundingBox();
        const center = new THREE.Vector3();
        geometry.boundingBox.getCenter(center);
        const size = new THREE.Vector3();
        geometry.boundingBox.getSize(size);

        const maxDim = Math.max(size.x, size.y, size.z);
        const fov = camera.fov * (Math.PI / 180);
        let cameraZ = Math.abs(maxDim / 2 / Math.tan(fov / 2));
        cameraZ *= 1.8; // Factor safety distance

        camera.position.set(center.x + cameraZ, center.y + cameraZ, center.z + cameraZ);
        camera.lookAt(center);
        controls.target.copy(center);

        // Handle window resizing dynamically
        window.addEventListener('resize', () => {
            camera.aspect = container.clientWidth / container.clientHeight;
            camera.updateProjectionMatrix();
            renderer.setSize(container.clientWidth, container.clientHeight);
        });

        // Animation Loop
        function animate() {
            requestAnimationFrame(animate);
            controls.update();
            renderer.render(scene, camera);
        }
        animate();
    </script>

    <footer class="py-8 text-center text-gray-500 text-xs border-t border-gray-200 mt-12">
        Generated by COMPAS Forge. Developed at the intersection of Computational Geometry and Robotic Fabrication.
    </footer>
</body>
</html>
"""

    # SOTA Template Replacement logic (Safe from f-string conflicts)
    html_content = html_template \
        .replace("__VOLUME_M3__", f"{preflight.get('volume_m3', 0.0):.6f}") \
        .replace("__ESTIMATED_MASS__", f"{preflight.get('estimated_mass_kg', 0.0):.3f}") \
        .replace("__BOUNDARY_EDGES_COUNT__", str(preflight.get('boundary_edges_count', 0))) \
        .replace("__VERTEX_COUNT__", str(diagnostics.get('vertex_count', 0))) \
        .replace("__FACE_COUNT__", str(diagnostics.get('face_count', 0))) \
        .replace("__EULER_CHARACTERISTIC__", str(preflight.get('euler_characteristic', 2))) \
        .replace("__GENUS__", str(preflight.get('genus', 0))) \
        .replace("__MAX_PLANARITY__", f"{preflight.get('max_planarity_deviation', 0.0):.6f}") \
        .replace("__MIN_QUALITY__", f"{preflight.get('min_face_quality', 1.0):.4f}") \
        .replace("__TIMELINE_HTML__", timeline_html) \
        .replace("__TOTAL_TIME__", f"{total_time:.3f}") \
        .replace("__WELD_COUNT__", str(repairs.get('welded_count', 0))) \
        .replace("__WELD_ROWS__", weld_rows) \
        .replace("__FLIP_COUNT__", str(repairs.get('flipped_count', 0))) \
        .replace("__FLIP_ROWS__", flip_rows) \
        .replace("__VERTICES_JSON__", json.dumps(vertices_data)) \
        .replace("__FACES_JSON__", json.dumps(faces_data)) \
        .replace("__BOUNDARY_EDGES_JSON__", json.dumps(boundary_edges_data)) \
        .replace("__IS_COMPLIANT_JSON__", json.dumps(preflight.get("is_compliant", False))) \
        .replace("__BOUNDS_X__", f"{preflight.get('bounds_x_dim', 0.0):.3f}") \
        .replace("__BOUNDS_Y__", f"{preflight.get('bounds_y_dim', 0.0):.3f}") \
        .replace("__BOUNDS_Z__", f"{preflight.get('bounds_z_dim', 0.0):.3f}") \
        .replace("__PROFILE_NAME__", preflight.get('profile_name', ''))

    # Injecting badges and icon conditions dynamically
    badge_style = f"bg-{preflight_color}-100 text-{preflight_color}-800 border border-{preflight_color}-200"
    html_content = html_content.replace("__PREFLIGHT_STATUS__", f'<span class="{badge_style} px-4 py-1.5 rounded-full text-xs font-bold">{preflight_status}</span>')
    
    # Icons and descriptors
    workspace_icon = "✔️" if preflight.get("fits_workspace") else "❌"
    watertight_icon = "✔️" if preflight.get("is_watertight") else "❌"
    watertight_desc = f"Unsafe Open Boundary Loops: {preflight.get('boundary_edges_count')}" if preflight.get('boundary_edges_count', 0) > 0 else "Perfect watertight envelope."
    compliant_icon = "✔️" if preflight.get("is_compliant") else "❌"

    html_content = html_content \
        .replace('<div class="flex-shrink-0 mt-1" id="check-workspace-icon"></div>', f'<div class="flex-shrink-0 mt-1">{workspace_icon}</div>') \
        .replace('<div class="flex-shrink-0 mt-1" id="check-watertight-icon"></div>', f'<div class="flex-shrink-0 mt-1">{watertight_icon}</div>') \
        .replace('<p class="text-xs text-gray-500 mt-1" id="check-watertight-desc"></p>', f'<p class="text-xs text-gray-500 mt-1">{watertight_desc}</p>') \
        .replace('<div class="flex-shrink-0 mt-1" id="check-compliant-icon"></div>', f'<div class="flex-shrink-0 mt-1">{compliant_icon}</div>')

    with open(filepath, 'w', encoding='utf-8') as f:
        f.write(html_content)