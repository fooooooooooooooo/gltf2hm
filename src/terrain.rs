// version(char)
// size(unsigned int)
// heightMap(heightMapSize * heightMapItemSize)
// layerMap(layerMapSize * layerMapItemSize)
// layerTextureMap(layerMapSize * layerMapItemSize)
// materialNames

use std::io;

use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

// const VERSION: u8 = 8;
const SIZE: usize = 2048;
const S: usize = SIZE * SIZE;

pub struct Terrain {
  pub height_map: Vec<u16>,
  pub layer_map: Vec<u8>,
  pub material_names: Vec<String>,
}

impl Terrain {
  pub fn new(height_map: Vec<u16>) -> Self {
    Self {
      height_map,
      layer_map: vec![0; S],
      material_names: vec!["warning_material".to_string()],
    }
  }

  pub fn write<W: io::Write>(&self, mut writer: W) -> io::Result<()> {
    writer.write_all(&(0x0800u16).to_be_bytes())?;
    writer.write_all(&(0x0800u16).to_be_bytes())?;
    writer.write_all(&[0x00])?;

    writer.write_all(
      &self
        .height_map
        .par_iter()
        .flat_map(|x| x.to_le_bytes())
        .collect::<Vec<_>>(),
    )?;
    writer.write_all(&self.layer_map)?;

    writer.write_all(&[0; S])?;
    writer.write_all(&[0; S])?;
    writer.write_all(&[0; S])?;
    writer.write_all(&[0; S])?;

    writer.write_all(&(self.material_names.len() as u16).to_le_bytes())?;
    writer.write_all(&[0x00])?;

    for name in &self.material_names {
      writer.write_all(&(name.len() as u16).to_be_bytes())?;
      writer.write_all(name.as_bytes())?;
    }

    Ok(())
  }
}
