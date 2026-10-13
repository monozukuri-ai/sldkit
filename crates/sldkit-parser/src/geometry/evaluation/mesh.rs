//! Mesh-derived quantities for one display tessellation. These are fallbacks
//! for callers without usable B-Rep edges; they never establish face ownership.
use std::collections::BTreeMap;

use sldkit_core::{EvaluatedBounds, EvaluatedMesh, GeometryTessellation};

pub(super) struct MeshEvaluation {
    pub mesh: EvaluatedMesh,
    /// Triangles with out-of-range or repeated indices; they are skipped.
    pub invalid_triangle_count: u64,
}

struct TriangleUse {
    normal: Option<[f64; 3]>,
    /// `true` when the triangle traverses the edge from its lower to its higher index.
    forward: bool,
}

#[allow(clippy::too_many_lines)]
pub(super) fn evaluate_mesh(
    tessellation: &GeometryTessellation,
    feature_angle_deg: f64,
) -> MeshEvaluation {
    let vertices = &tessellation.vertices;
    let vertex_count = vertices.len();
    let mut invalid = 0_u64;
    let mut area = 0.0_f64;
    let mut volume = 0.0_f64;
    let mut valid_triangles = 0_u64;
    let mut edges: BTreeMap<[u32; 2], Vec<TriangleUse>> = BTreeMap::new();
    for triangle in &tessellation.triangles {
        let indices = triangle.map(|index| usize::try_from(index).ok());
        let (Some(a), Some(b), Some(c)) = (indices[0], indices[1], indices[2]) else {
            invalid = invalid.saturating_add(1);
            continue;
        };
        if a >= vertex_count || b >= vertex_count || c >= vertex_count || a == b || b == c || a == c
        {
            invalid = invalid.saturating_add(1);
            continue;
        }
        let (p0, p1, p2) = (vertices[a], vertices[b], vertices[c]);
        if [p0, p1, p2]
            .iter()
            .any(|point| point.iter().any(|value| !value.is_finite()))
        {
            invalid = invalid.saturating_add(1);
            continue;
        }
        valid_triangles = valid_triangles.saturating_add(1);
        let facet = cross(sub(p1, p0), sub(p2, p0));
        let doubled_area = norm(facet);
        area += doubled_area / 2.0;
        volume += dot(p0, cross(p1, p2)) / 6.0;
        let normal = (doubled_area > 0.0).then(|| scale(facet, 1.0 / doubled_area));
        for (from, to) in [
            (triangle[0], triangle[1]),
            (triangle[1], triangle[2]),
            (triangle[2], triangle[0]),
        ] {
            let key = [from.min(to), from.max(to)];
            edges.entry(key).or_default().push(TriangleUse {
                normal,
                forward: from < to,
            });
        }
    }

    let mut closed = valid_triangles > 0 && invalid == 0;
    let mut feature_edges = Vec::new();
    let mut boundary_edges = Vec::new();
    let cosine_threshold = feature_angle_deg.to_radians().cos();
    for (key, uses) in &edges {
        match uses.as_slice() {
            [_] => {
                closed = false;
                boundary_edges.push(*key);
            }
            [first, second] => {
                if first.forward == second.forward {
                    closed = false;
                }
                if let (Some(left), Some(right)) = (first.normal, second.normal)
                    && dot(left, right) <= cosine_threshold
                {
                    feature_edges.push(*key);
                }
            }
            _ => closed = false,
        }
    }

    let bounds = (vertex_count > 0
        && vertices
            .iter()
            .all(|point| point.iter().all(|value| value.is_finite())))
    .then(|| {
        let mut min = [f64::INFINITY; 3];
        let mut max = [f64::NEG_INFINITY; 3];
        for point in vertices {
            for ((low, high), value) in min.iter_mut().zip(max.iter_mut()).zip(point) {
                *low = low.min(*value);
                *high = high.max(*value);
            }
        }
        EvaluatedBounds { min, max }
    });

    MeshEvaluation {
        mesh: EvaluatedMesh {
            tessellation_id: tessellation.id.clone(),
            body_id: tessellation.body_id.clone(),
            vertex_count: u64::try_from(vertex_count).unwrap_or(u64::MAX),
            triangle_count: u64::try_from(tessellation.triangles.len()).unwrap_or(u64::MAX),
            bounds,
            area_mm2: area,
            is_closed: (valid_triangles > 0).then_some(closed),
            signed_volume_mm3: closed.then_some(volume),
            feature_edges,
            boundary_edges,
            feature_angle_deg,
        },
        invalid_triangle_count: invalid,
    }
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

fn scale(a: [f64; 3], factor: f64) -> [f64; 3] {
    [a[0] * factor, a[1] * factor, a[2] * factor]
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use sldkit_core::{GeometryEntityProvenance, GeometryExactness};

    /// Outward counter-clockwise unit cube spanning the origin to `(size, size, size)`.
    pub(in crate::geometry::evaluation) fn cube_tessellation(
        id: &str,
        size: f64,
    ) -> GeometryTessellation {
        let vertices = vec![
            [0.0, 0.0, 0.0],
            [size, 0.0, 0.0],
            [size, size, 0.0],
            [0.0, size, 0.0],
            [0.0, 0.0, size],
            [size, 0.0, size],
            [size, size, size],
            [0.0, size, size],
        ];
        let triangles = vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ];
        GeometryTessellation {
            id: id.to_owned(),
            body_id: None,
            face_ids: Vec::new(),
            chordal_deflection: None,
            source_object: None,
            vertices,
            triangles,
            feature_edges: Vec::new(),
            strip_lengths: Vec::new(),
            normals: Vec::new(),
            corner_normals: Vec::new(),
            triangle_groups: Vec::new(),
            texture_assignments: Vec::new(),
            channels: Vec::new(),
            provenance: GeometryEntityProvenance {
                stream: None,
                offset: None,
                tag: None,
                exactness: GeometryExactness::Unknown,
                field_exactness: BTreeMap::new(),
            },
        }
    }

    #[test]
    fn closed_cube_has_volume_area_and_twelve_feature_edges() {
        let evaluated = evaluate_mesh(&cube_tessellation("cube", 1.0), 30.0);
        let mesh = evaluated.mesh;
        assert_eq!(evaluated.invalid_triangle_count, 0);
        assert_eq!(mesh.is_closed, Some(true));
        assert!((mesh.signed_volume_mm3.unwrap_or(0.0) - 1.0).abs() < 1e-12);
        assert!((mesh.area_mm2 - 6.0).abs() < 1e-12);
        assert_eq!(mesh.feature_edges.len(), 12);
        assert!(mesh.boundary_edges.is_empty());
        assert_eq!(
            mesh.bounds,
            Some(EvaluatedBounds {
                min: [0.0; 3],
                max: [1.0; 3]
            })
        );
        let flat = evaluate_mesh(&cube_tessellation("cube", 1.0), 91.0);
        assert!(flat.mesh.feature_edges.is_empty());
    }

    #[test]
    fn open_inconsistent_and_invalid_meshes_are_not_closed() {
        let mut open = cube_tessellation("open", 2.0);
        let _ = open.triangles.pop();
        let evaluated = evaluate_mesh(&open, 30.0);
        assert_eq!(evaluated.mesh.is_closed, Some(false));
        assert_eq!(evaluated.mesh.signed_volume_mm3, None);
        assert_eq!(evaluated.mesh.boundary_edges.len(), 3);

        let mut flipped = cube_tessellation("flipped", 1.0);
        flipped.triangles[0] = [0, 1, 2];
        assert_eq!(evaluate_mesh(&flipped, 30.0).mesh.is_closed, Some(false));

        let mut invalid = cube_tessellation("invalid", 1.0);
        invalid.triangles.push([0, 0, 9]);
        let evaluated = evaluate_mesh(&invalid, 30.0);
        assert_eq!(evaluated.invalid_triangle_count, 1);
        assert_eq!(evaluated.mesh.is_closed, Some(false));

        let mut empty = cube_tessellation("empty", 1.0);
        empty.triangles.clear();
        assert_eq!(evaluate_mesh(&empty, 30.0).mesh.is_closed, None);
    }
}
