use anyhow::Context;
use gltf::buffer::Data;
use gltf::Mesh;
use rayon::iter::{IntoParallelIterator, ParallelIterator};

// aabb
pub fn find_bounds(buffers: &[Data], meshes: &[Mesh]) -> anyhow::Result<([f32; 3], [f32; 3])> {
  let mut min = [f32::MAX, f32::MAX, f32::MAX];
  let mut max = [f32::MIN, f32::MIN, f32::MIN];

  for mesh in meshes {
    for primitive in mesh.primitives() {
      let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
      let positions = reader.read_positions().context("failed to read primitive positions")?;

      for position in positions {
        if position[0] < min[0] {
          min[0] = position[0];
        }

        if position[1] < min[1] {
          min[1] = position[1];
        }

        if position[2] < min[2] {
          min[2] = position[2];
        }

        if position[0] > max[0] {
          max[0] = position[0];
        }

        if position[1] > max[1] {
          max[1] = position[1];
        }

        if position[2] > max[2] {
          max[2] = position[2];
        }
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

    if count > 0 {
      let start = line[i];
      let end = line[i + count];

      let step = (end - start) / (count + 1) as f32;

      for j in 1..=count {
        line[i + j] = start + step * j as f32;
      }
    }
  }
}
