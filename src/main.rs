use std::env::args;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use anyhow::Context;
use gltf2hm::terrain::Terrain;
use gltf2hm::util::{find_bounds, interpolate};
use image::codecs::png::PngEncoder;
use image::{ColorType, ImageEncoder};

fn main() -> anyhow::Result<()> {
  let binary_name = args().next().context("wtf")?;
  let binary_name = Path::new(&binary_name)
    .file_name()
    .context("wtf")?
    .to_str()
    .context("wtf")?;
  let path = args().nth(1).unwrap_or_else(|| panic!("Usage: {binary_name} <file>"));

  let (document, buffers, _) = gltf::import(path)?;

  let meshes = document.meshes().collect::<Vec<_>>();

  // find bounding volume of all meshes
  let (min, max) = find_bounds(&buffers, &meshes)?;

  println!(
    r#"bounds: 
  -X: {:>+0width$.prec$} -Y: {:>+0width$.prec$} -Z: {:>+0width$.prec$}
  +X: {:>+0width$.prec$} +Y: {:>+0width$.prec$} +Z: {:>+0width$.prec$}
"#,
    min[0],
    min[1],
    min[2],
    max[0],
    max[1],
    max[2],
    width = 1,
    prec = 8
  );

  println!(
    r#"size: 
  X: {:>0width$.prec$}
  Y: {:>0width$.prec$}
  Z: {:>0width$.prec$}
"#,
    max[0] - min[0],
    max[1] - min[1],
    max[2] - min[2],
    width = 1,
    prec = 8
  );

  let height = 2048;
  let width = 2048;
  let mut map = vec![0f32; height * width];

  println!("mapping");

  // loop all vertices in all meshes, and find the closest one to each point in
  // the map if the height is higher than the current height, replace it
  for mesh in meshes {
    for primitive in mesh.primitives() {
      let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
      let positions = reader.read_positions().context("failed to read primitive positions")?;

      for position in positions {
        // map the vertex position to the map position, rounding to the nearest
        // integer
        let x = ((position[0] - min[0]) / (max[0] - min[0]) * width as f32).round() as usize;
        let y = ((position[2] - min[2]) / (max[2] - min[2]) * height as f32).round() as usize;

        if x >= width || y >= height {
          continue;
        }

        let vertex_height = position[1];
        // scale the height to the range [0, 1]
        let vertex_height = (vertex_height - min[1]) / (max[1] - min[1]);

        if map[y * width + x] < vertex_height {
          map[y * width + x] = vertex_height;
        }
      }
    }
  }

  println!("interpolating");
  interpolate(&mut map, width, height);

  // write to image for testing
  #[cfg(debug_assertions)]
  {
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

  let map = map
    .iter()
    .map(|v| (v * u16::MAX as f32) as u16)
    .collect::<Vec<_>>();

  println!("writing terrain.ter");
  let mut file = BufWriter::new(File::create("terrain.ter")?);

  let terrain = Terrain::new(map);

  terrain.write(&mut file)?;
  println!("done");

  Ok(())
}
