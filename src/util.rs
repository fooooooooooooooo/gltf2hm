use anyhow::Context;
use gltf::buffer::Data;
use gltf::Mesh;
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use crate::{Vec2, Vec3};

// aabb
pub fn find_bounds(buffers: &[Data], meshes: &[Mesh]) -> anyhow::Result<(Vec3, Vec3)> {
  let mut min = Vec3::new(f32::MAX, f32::MAX, f32::MAX);
  let mut max = Vec3::new(f32::MIN, f32::MIN, f32::MIN);

  for mesh in meshes {
    for primitive in mesh.primitives() {
      let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
      let positions = reader
        .read_positions()
        .context("failed to read primitive positions")?
        .map(Vec3::from);

      for position in positions {
        min.x = min.x.min(position.x);
        min.y = min.y.min(position.y);
        min.z = min.z.min(position.z);

        max.x = max.x.max(position.x);
        max.y = max.y.max(position.y);
        max.z = max.z.max(position.z);
      }
    }
  }

  Ok((min, max))
}

/// bilinear interpolation in the elements with value 0.0
pub fn interpolate(map: &mut [f32], width: usize, height: usize) {
  let temp = (0..height)
    .into_par_iter()
    .flat_map(|y| {
      let mut section = map[y * width..(y + 1) * width].to_vec();

      interpolate_line(&mut section);

      section
    })
    .collect::<Vec<_>>();

  // repeat for y
  let temp = (0..width)
    .into_par_iter()
    .flat_map(|x| {
      let mut section = temp[x..].iter().step_by(width).copied().collect::<Vec<_>>();

      interpolate_line(&mut section);

      section
    })
    .collect::<Vec<_>>();

  // transpose
  let temp = (0..width)
    .into_par_iter()
    .flat_map(|x| temp[x..].iter().step_by(width).copied().collect::<Vec<_>>())
    .collect::<Vec<_>>();

  map.copy_from_slice(&temp);
}

/// broken but looks cool
pub fn interpolate_scuffed(map: &mut [f32], width: usize, height: usize) {
  for y in 0..height {
    for x in 0..width {
      if map[y * width + x] == 0.0 {
        let mut total = 0.0;
        let mut count = 0;

        for dy in -1..=1 {
          for dx in -1..=1 {
            let x = x as isize + dx;
            let y = y as isize + dy;

            if x < 0 || y < 0 || x >= width as isize || y >= height as isize {
              continue;
            }

            let x = x as usize;
            let y = y as usize;

            if map[y * width + x] != 0.0 {
              total += map[y * width + x];
              count += 1;
            }
          }
        }

        if count > 0 {
          map[y * width + x] = total / count as f32;
        }
      }
    }
  }
}

fn interpolate_line(line: &mut [f32]) {
  for i in 0..line.len() - 1 {
    if line[i] == 0.0 {
      continue;
    }

    let mut count = 1;

    while i + count < line.len() - 1 && line[i + count] == 0.0 {
      count += 1;
    }

    if count == 1 || count >= 48 || line[i + count] == 0.0 {
      continue;
    }

    let start = line[i];
    let end = line[i + count];

    let step = (end - start) / (count + 1) as f32;

    for j in 1..=count {
      line[i + j] = start + step * j as f32;
    }
  }
}

// calculate height of vec2 point p in triangle va, vb, vc
pub fn calculate_height(p: Vec2, va: Vec3, vb: Vec3, vc: Vec3) -> f32 {
  let a = -(vc.z * vb.y - va.z * vb.y - vc.z * va.y + va.y * vb.z + vc.y * va.z - vb.z * vc.y);
  let b = va.z * vc.x + vb.z * va.x + vc.z * vb.x - vb.z * vc.x - va.z * vb.x - vc.z * va.x;
  let c = vb.y * vc.x + va.y * vb.x + vc.y * va.x - va.y * vc.x - vb.y * va.x - vb.x * vc.y;
  let d = -a * va.x - b * va.y - c * va.z;

  -(a * p.x + c * p.y + d) / b
}

pub fn vertex_to_map(v: Vec3, min: Vec3, max: Vec3, width: usize, height: usize) -> Vec2 {
  Vec2::new(
    ((v.x - min.x) / (max.x - min.x) * width as f32).ceil(),
    ((v.z - min.z) / (max.z - min.z) * height as f32).ceil(),
  )
}

pub fn map_to_vertex(p: Vec2, min: Vec3, max: Vec3, width: usize, height: usize) -> Vec2 {
  Vec2::new(
    p.x / width as f32 * (max.x - min.x) + min.x,
    p.y / height as f32 * (max.z - min.z) + min.z,
  )
}

pub fn point_in_triangle(p: Vec2, v1: Vec2, v2: Vec2, v3: Vec2) -> bool {
  let area = 0.5 * (-v2.y * v3.x + v1.y * (-v2.x + v3.x) + v1.x * (v2.y - v3.y) + v2.x * v3.y);

  let s = 1.0 / (2.0 * area) * (v1.y * v3.x - v1.x * v3.y + (v3.y - v1.y) * p.x + (v1.x - v3.x) * p.y);
  let t = 1.0 / (2.0 * area) * (v1.x * v2.y - v1.y * v2.x + (v1.y - v2.y) * p.x + (v2.x - v1.x) * p.y);

  s > 0.0 && t > 0.0 && 1.0 - s - t > 0.0
}

pub fn point_in_triangle_1(p: Vec2, p1: Vec2, p2: Vec2, p3: Vec2) -> bool {
  const TOLERANCE: f32 = -0.005;

  let alpha = ((p2.y - p3.y) * (p.x - p3.x) + (p3.x - p2.x) * (p.y - p3.y))
    / ((p2.y - p3.y) * (p1.x - p3.x) + (p3.x - p2.x) * (p1.y - p3.y));

  let beta = ((p3.y - p1.y) * (p.x - p3.x) + (p1.x - p3.x) * (p.y - p3.y))
    / ((p2.y - p3.y) * (p1.x - p3.x) + (p3.x - p2.x) * (p1.y - p3.y));

  let gamma = 1.0 - alpha - beta;

  alpha > TOLERANCE && beta > TOLERANCE && gamma > TOLERANCE
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_point_in_triangle() {
    let triangle = [
      Vec3::new(0.0, 0.0, 0.0).xy(),
      Vec3::new(1.0, 0.0, 0.0).xy(),
      Vec3::new(0.5, 1.0, 1.0).xy(),
    ];

    assert!(point_in_triangle(
      Vec2::new(0.5, 0.5),
      triangle[0],
      triangle[1],
      triangle[2]
    ));

    assert!(!point_in_triangle(
      Vec2::new(1.0, 1.0),
      triangle[0],
      triangle[1],
      triangle[2]
    ));

    assert_eq!(
      point_in_triangle(Vec2::new(0.5, 0.5), triangle[0], triangle[1], triangle[2]),
      point_in_triangle_1(Vec2::new(0.5, 0.5), triangle[0], triangle[1], triangle[2])
    );

    assert_eq!(
      point_in_triangle(Vec2::new(1.0, 1.0), triangle[0], triangle[1], triangle[2]),
      point_in_triangle_1(Vec2::new(1.0, 1.0), triangle[0], triangle[1], triangle[2])
    );
  }

  #[test]
  fn test_calculate_height() {
    let flat_triangle = [
      Vec3::new(0.0, 0.0, 0.0),
      Vec3::new(1.0, 0.0, 0.0),
      Vec3::new(0.0, 0.0, 1.0),
    ];

    assert_eq!(
      calculate_height(
        Vec2::new(0.5, 0.5),
        flat_triangle[0],
        flat_triangle[1],
        flat_triangle[2]
      ),
      0.0
    );

    let triangle = [
      Vec3::new(0.0, 0.0, 0.0),
      Vec3::new(1.0, 0.0, 0.0),
      Vec3::new(0.5, 1.0, 1.0),
    ];

    assert_eq!(
      calculate_height(Vec2::new(0.5, 0.5), triangle[0], triangle[1], triangle[2]),
      0.5
    );
  }
}
