use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use anyhow::Context;
use clap::Parser;
use gltf2hm::args::Args;
use gltf2hm::terrain::Terrain;
use gltf2hm::util::{calculate_height, find_bounds, map_to_vertex, point_in_triangle_1, vertex_to_map};
use gltf2hm::Vec3;
use image::codecs::png::PngEncoder;
use image::{ColorType, ImageEncoder};
use nalgebra::Vector2;

fn main() -> anyhow::Result<()> {
  let args = Args::parse();

  let path = Path::new(&args.input);

  if !path.exists() {
    println!("file {path:?} does not exist");
    return Ok(());
  }

  println!("loading {path:?}");

  let (document, buffers, _) = gltf::import(path)?;

  let meshes = document.meshes().collect::<Vec<_>>();

  // find bounding volume of all meshes
  let (min, max) = find_bounds(&buffers, &meshes)?;

  println!(
    r#"bounds: 
  -X: {:>+0width$.prec$} -Y: {:>+0width$.prec$} -Z: {:>+0width$.prec$}
  +X: {:>+0width$.prec$} +Y: {:>+0width$.prec$} +Z: {:>+0width$.prec$}
"#,
    min.x,
    min.y,
    min.z,
    max.x,
    max.y,
    max.z,
    width = 1,
    prec = 8
  );

  println!(
    r#"size: 
  X: {:>0width$.prec$}
  Y: {:>0width$.prec$}
  Z: {:>0width$.prec$}
"#,
    max.x - min.x,
    max.y - min.y,
    max.z - min.z,
    width = 1,
    prec = 8
  );

  let height = args.size;
  let width = args.size;
  let mut map = vec![0f32; height * width];

  match meshes.len() {
    0 => {
      println!("no meshes found");
      return Ok(());
    }
    1 => println!("mapping 1 mesh"),
    n => println!("mapping {n} meshes"),
  }

  // loop all vertices in all meshes, and find the closest one to each point in
  // the map if the height is higher than the current height, replace it
  for mesh in meshes {
    for primitive in mesh.primitives() {
      let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
      let positions: Vec<_> = reader
        .read_positions()
        .context("failed to read primitive positions")?
        .map(Vec3::from)
        .collect();

      let indices: Vec<_> = reader
        .read_indices()
        .context("failed to read primitive indices")?
        .into_u32()
        .collect();

      for i in (0..indices.len()).step_by(3) {
        let vertex_indices = [indices[i] as usize, indices[i + 1] as usize, indices[i + 2] as usize];

        let vertices = [
          positions[vertex_indices[0]],
          positions[vertex_indices[1]],
          positions[vertex_indices[2]],
        ];

        let mut v_bbox_min = vertices[0];
        let mut v_bbox_max = vertices[0];

        for vertex in vertices {
          v_bbox_min.x = v_bbox_min.x.min(vertex.x);
          v_bbox_min.z = v_bbox_min.z.min(vertex.z);

          v_bbox_max.x = v_bbox_max.x.max(vertex.x);
          v_bbox_max.z = v_bbox_max.z.max(vertex.z);
        }

        // vertex coordinates to map coordinates
        let map_bbox_min = vertex_to_map(v_bbox_min, min, max, width, height);
        let map_bbox_max = vertex_to_map(v_bbox_max, min, max, width, height);

        let map_bbox_min = Vector2::new(map_bbox_min.x as usize, map_bbox_min.y as usize);
        let map_bbox_max = Vector2::new(map_bbox_max.x as usize, map_bbox_max.y as usize);

        let map_vertices = [
          vertex_to_map(vertices[0], min, max, width, height),
          vertex_to_map(vertices[1], min, max, width, height),
          vertex_to_map(vertices[2], min, max, width, height),
        ];

        // for each point in the bounding box, check if it is inside the triangle
        // if it is, interpolate the height and replace the current height if it
        // is higher
        for y in map_bbox_min.y..map_bbox_max.y {
          for x in map_bbox_min.x..map_bbox_max.x {
            let p = Vector2::new(x as f32, y as f32);

            if point_in_triangle_1(p, map_vertices[0].xy(), map_vertices[1].xy(), map_vertices[2].xy()) {
              // calculate the height of the point with barycentric coordinates
              let vertex_height = calculate_height(
                map_to_vertex(p, min, max, width, height),
                vertices[0],
                vertices[1],
                vertices[2],
              );

              // normalize the height to the range [0, 1]
              let vertex_height = (vertex_height - min.y) / (max.y - min.y);

              let value = &mut map[y * width + x];
              *value = value.max(vertex_height);
            }
          }
        }
      }
    }
  }

  println!("fixing holes");
  fix_holes(&mut map, width, height);

  // println!("interpolating");
  // interpolate(&mut map, width, height);

  if args.smooth != 0.0 {
    println!("smoothing");
    smooth(&mut map, width, height, args.smooth);
  }

  println!("unspiking");
  unspike(&mut map, width, height);

  if args.flip_x {
    map = map
      .chunks_exact(width)
      .flat_map(|row| row.iter().rev().copied().collect::<Vec<_>>())
      .collect();
  }

  if args.flip_y {
    map = map.chunks_exact(width).rev().flatten().copied().collect();
  }

  // write to image for testing
  if args.heightmap {
    println!("writing heightmap.png");

    let mut file = BufWriter::new(File::create("heightmap.png")?);

    PngEncoder::new(&mut file).write_image(
      &map.iter().map(|x| (x * 255.0) as u8).collect::<Vec<_>>(),
      width as u32,
      height as u32,
      ColorType::L8,
    )?;

    println!("done");
  }

  let map = map.iter().map(|v| (v * u16::MAX as f32) as u16).collect::<Vec<_>>();

  let output = args.output.unwrap_or_else(|| "terrain.ter".into());

  println!("writing {output}");
  let mut file = BufWriter::new(File::create(output)?);

  let terrain = Terrain::new(map);

  terrain.write(&mut file)?;
  println!("done");

  Ok(())
}

/// if a point has a value of 0 or a value of 1 and is surrounded by at least 6
/// points with a value > 0, replace it with the average of the surrounding
/// points
fn fix_holes(map: &mut [f32], width: usize, height: usize) {
  for y in 0..height {
    for x in 0..width {
      if map[y * width + x] > 0.0 && map[y * width + x] < 1.0 {
        continue;
      }

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

          if map[y * width + x] > 0.0 && map[y * width + x] < 1.0 {
            total += map[y * width + x];
            count += 1;
          }

          if count >= 6 {
            break;
          }
        }
      }

      if count >= 6 {
        map[y * width + x] = total / count as f32;
      }
    }
  }
}

/// smooth sharp changes in height
///
/// if a point has a very low or very high value compared to its neighbors,
/// replace it with the average of its neighbors
pub fn smooth(map: &mut [f32], width: usize, height: usize, amount: f32) {
  for y in 0..height {
    for x in 0..width {
      let mut total = 0.0;
      let mut count = 0u8;
      let mut edge_count = 0u8;

      for dy in -1..=1 {
        for dx in -1..=1 {
          let x = x as isize + dx;
          let y = y as isize + dy;

          if x < 0 || y < 0 || x >= width as isize || y >= height as isize {
            continue;
          }

          let x = x as usize;
          let y = y as usize;

          let value = map[y * width + x];

          if value == 0.0 || value == 1.0 {
            edge_count += 1;
            continue;
          }

          total += value;
          count += 1;
        }
      }

      if edge_count > 2 || count == 0 {
        continue;
      }

      let average = total / count as f32;

      let value = &mut map[y * width + x];

      if *value - average > amount || average - *value > amount {
        *value = average;
      }
    }
  }
}

/// remove sharp spikes in single points by checking if difference to all
/// neighbors is above a threshold
pub fn unspike(map: &mut [f32], width: usize, height: usize) {
  const DIFF: f32 = 0.0001;

  let i2d = |x: usize, y: usize| y * width + x;

  for y in 0..height {
    for x in 0..width {
      if x == 0 || y == 0 || x == width - 1 || y == height - 1 {
        continue;
      }

      let neighbors = [
        map[i2d(x, y - 1)],
        map[i2d(x - 1, y)],
        map[i2d(x + 1, y)],
        map[i2d(x, y + 1)],
      ];

      let value = &mut map[y * width + x];

      if *value == 0.0 || *value == 1.0 || neighbors.iter().all(|n| (*n - *value).abs() > DIFF) {
        *value = neighbors.iter().sum::<f32>() / neighbors.len() as f32;
      }
    }
  }
}
