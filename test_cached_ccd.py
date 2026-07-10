import compas_forge
import math

print("1. Cleaning mesh registry...")
compas_forge.clear_mesh_registry()

# تعریف هندسه مش اول (یک جعبه ساده ۲x۲x۲)
vertices_1 = [
    -1.0, -1.0, -1.0,  # 0
     1.0, -1.0, -1.0,  # 1
     1.0,  1.0, -1.0,  # 2
    -1.0,  1.0, -1.0,  # 3
    -1.0, -1.0,  1.0,  # 4
     1.0, -1.0,  1.0,  # 5
     1.0,  1.0,  1.0,  # 6
    -1.0,  1.0,  1.0   # 7
]

# نمایه‌های وجه‌ها (Face Indices) برای یک جعبه (۶ وجه ۴ رأسی)
face_indices_1 = [
    0, 3, 2, 1,  # Bottom
    4, 5, 6, 7,  # Top
    0, 1, 5, 4,  # Front
    1, 2, 6, 5,  # Right
    2, 3, 7, 6,  # Back
    3, 0, 4, 7   # Left
]

# ایندکس‌های شروع هر وجه در آرایه face_indices_1
face_offsets_1 = [0, 4, 8, 12, 16, 20, 24]

# تعریف مش دوم (عیناً مشابه مش اول)
vertices_2 = vertices_1.copy()
face_indices_2 = face_indices_1.copy()
face_offsets_2 = face_offsets_1.copy()

print("2. Registering meshes in Rust core...")
# ثبت مش‌ها در حافظه کش Rust
msg1 = compas_forge.register_mesh("robot_link_1", vertices_1, face_indices_1, face_offsets_1)
msg2 = compas_forge.register_mesh("obstacle_box", vertices_2, face_indices_2, face_offsets_2)
print("   -", msg1)
print("   -", msg2)

# تعریف حالت‌های حرکتی (Pose: [x, y, z, qx, qy, qz, qw])
# مش اول (robot_link_1) از نقطه x=-3 شروع شده و به x=3 می‌رود (دوران ۹۰ درجه حول محور Z)
pose1_start = [-3.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0]
pose1_end   = [ 3.0, 0.0, 0.0, 0.0, 0.0, 0.7071, 0.7071]

# مش دوم (obstacle_box) ثابت در مرکز مختصات (x=0, y=0, z=0) ایستاده است
pose2_start = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0]
pose2_end   = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0]

print("3. Running Cached Swept Collision Detection...")
# اجرای متد برخورد مداوم فوق‌سریع بدون بارگذاری مش‌ها
import json
import time

t0 = time.perf_counter()
result_str = compas_forge.check_swept_collision_cached(
    "robot_link_1", pose1_start, pose1_end,
    "obstacle_box", pose2_start, pose2_end
)
t1 = time.perf_counter()

result = json.loads(result_str)
print(f"   - Done in {(t1 - t0)*1000:.3f} ms!")
print("   - Has Collision:", result["has_collision"])
if result["has_collision"]:
    print(f"   - Time of Impact (TOI): {result['time_of_impact']:.6f}")
    print(f"   - Collision Normal: {result['normal_a']}")