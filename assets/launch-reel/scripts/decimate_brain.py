#!/usr/bin/env python3
"""
Decimate MRI brain mesh (brainder.org pial surface OBJ) for wireframe rendering.

Downloads the pial surface OBJ, performs vertex clustering decimation,
and outputs inline JS arrays for embedding in HyperFrames compositions.

Source: brainder.org CC BY-SA 3.0
Author: Anderson Winkler

Usage:
    python3 decimate_brain.py [--grid SIZE] [--output FILE]

Default grid_size=30 yields ~120 vertices, ~467 edges.
Lower grid = more vertices/detail. Higher grid = fewer vertices/faster.
"""
import math, json, argparse, tempfile, subprocess, os

BRAIN_OBJ_URL = "https://s3.us-east-2.amazonaws.com/brainder/software/brain4blender/smallfiles/pial_Full_obj.tar.bz2"

def read_obj(filepath):
    vertices = []
    faces = []
    with open(filepath, 'r') as f:
        for line in f:
            if line.startswith('v '):
                parts = line.strip().split()
                vertices.append((float(parts[1]), float(parts[2]), float(parts[3])))
            elif line.startswith('f '):
                parts = line.strip().split()[1:]
                indices = [int(p.split('/')[0]) - 1 for p in parts]
                faces.append(indices)
    return vertices, faces

def decimate_mesh(vertices, faces, grid_size=30):
    xs = [v[0] for v in vertices]
    ys = [v[1] for v in vertices]
    zs = [v[2] for v in vertices]
    min_x, max_x = min(xs), max(xs)
    min_y, max_y = min(ys), max(ys)
    min_z, max_z = min(zs), max(zs)

    vertex_to_cell = {}
    cell_vertices = {}

    for i, v in enumerate(vertices):
        cx = int((v[0] - min_x) / grid_size)
        cy = int((v[1] - min_y) / grid_size)
        cz = int((v[2] - min_z) / grid_size)
        cell = (cx, cy, cz)
        vertex_to_cell[i] = cell
        if cell not in cell_vertices:
            cell_vertices[cell] = []
        cell_vertices[cell].append(i)

    new_vertices = []
    cell_to_new_idx = {}
    for cell, vindices in cell_vertices.items():
        idx = len(new_vertices)
        cell_to_new_idx[cell] = idx
        avg_x = sum(vertices[vi][0] for vi in vindices) / len(vindices)
        avg_y = sum(vertices[vi][1] for vi in vindices) / len(vindices)
        avg_z = sum(vertices[vi][2] for vi in vindices) / len(vindices)
        new_vertices.append((avg_x, avg_y, avg_z))

    new_faces = []
    for face in faces:
        new_face = []
        seen = set()
        for fidx in face:
            cell = vertex_to_cell[fidx]
            new_idx = cell_to_new_idx[cell]
            if new_idx not in seen:
                new_face.append(new_idx)
                seen.add(new_idx)
        if len(new_face) >= 3:
            new_faces.append(new_face)

    edge_set = set()
    for face in new_faces:
        for i in range(len(face)):
            a, b = face[i], face[(i+1) % len(face)]
            edge_set.add((min(a,b), max(a,b)))

    return new_vertices, list(edge_set)

def main():
    parser = argparse.ArgumentParser(description="Decimate MRI brain mesh for HyperFrames")
    parser.add_argument("--grid", type=float, default=30, help="Grid size for vertex clustering (default: 30)")
    parser.add_argument("--output", default="brain_mesh_data.js", help="Output JS file")
    parser.add_argument("--obj-dir", default=None, help="Directory with lh.pial.obj and rh.pial.obj (skips download)")
    args = parser.parse_args()

    if args.obj_dir:
        lh_path = os.path.join(args.obj_dir, "lh.pial.obj")
        rh_path = os.path.join(args.obj_dir, "rh.pial.obj")
    else:
        import urllib.request
        tmpdir = tempfile.mkdtemp(prefix="brain_")
        archive = os.path.join(tmpdir, "pial_Full_obj.tar.bz2")
        print(f"Downloading {BRAIN_OBJ_URL}...")
        urllib.request.urlretrieve(BRAIN_OBJ_URL, archive)
        print("Extracting...")
        subprocess.run(["tar", "xjf", archive, "-C", tmpdir], check=True)
        lh_path = os.path.join(tmpdir, "pial_Full_obj", "lh.pial.obj")
        rh_path = os.path.join(tmpdir, "pial_Full_obj", "rh.pial.obj")

    print(f"Loading LH: {lh_path}")
    lh_v, lh_f = read_obj(lh_path)
    print(f"  {len(lh_v)} vertices, {len(lh_f)} faces")

    print(f"Loading RH: {rh_path}")
    rh_v, rh_f = read_obj(rh_path)
    print(f"  {len(rh_v)} vertices, {len(rh_f)} faces")

    print(f"Decimating with grid_size={args.grid}...")
    lh_nv, lh_e = decimate_mesh(lh_v, lh_f, grid_size=args.grid)
    rh_nv, rh_e = decimate_mesh(rh_v, rh_f, grid_size=args.grid)

    # Merge hemispheres
    offset = len(lh_nv)
    all_v = list(lh_nv) + list(rh_nv)
    all_e = list(lh_e) + [(e[0]+offset, e[1]+offset) for e in rh_e]

    # Deduplicate edges
    edge_set = set()
    for e in all_e:
        edge_set.add((min(e), max(e)))
    all_e = list(edge_set)

    # Normalize to [-1, 1]
    cx = sum(v[0] for v in all_v) / len(all_v)
    cy = sum(v[1] for v in all_v) / len(all_v)
    cz = sum(v[2] for v in all_v) / len(all_v)
    max_r = max(math.sqrt((v[0]-cx)**2 + (v[1]-cy)**2 + (v[2]-cz)**2) for v in all_v)
    norm_v = [((v[0]-cx)/max_r, (v[1]-cy)/max_r, (v[2]-cz)/max_r) for v in all_v]

    print(f"Result: {len(norm_v)} vertices, {len(all_e)} edges")

    # Output as JS
    v_js = "[" + ",".join(f"[{v[0]:.4f},{v[1]:.4f},{v[2]:.4f}]" for v in norm_v) + "]"
    e_js = "[" + ",".join(f"[{e[0]},{e[1]}]" for e in all_e) + "]"

    with open(args.output, 'w') as f:
        f.write(f"// Brain mesh: {len(norm_v)} vertices, {len(all_e)} edges\n")
        f.write(f"// Decimated from MRI pial surface (brainder.org, CC BY-SA 3.0)\n")
        f.write(f"// Grid size: {args.grid}\n")
        f.write(f"var brainV = {v_js};\nvar brainE = {e_js};\n")

    print(f"Written to {args.output}")

if __name__ == "__main__":
    main()