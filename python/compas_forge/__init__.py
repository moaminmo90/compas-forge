import json
from compas_forge._core import validate_compas_json, detect_clashes_json, fix_mesh_json, run_preflight_json
from compas_forge.reporter import generate_html_report

__version__ = "0.1.0"

def verify_file(filepath: str) -> dict:
    with open(filepath, 'r', encoding='utf-8') as f:
        raw_data = f.read()
    report_raw = validate_compas_json(raw_data)
    return json.loads(report_raw)

def check_assembly_clashes(files_map: dict, clearance_tolerance: float = 0.0) -> list:
    """
    Accepts a dictionary of { "file_name": "raw_json_string" } and the clearance tolerance,
    and returns a list of clashing file pairs using the SOTA Rust engine.
    """
    items = list(files_map.items())
    # Corrected line: passed the parameter to Rust FFI
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