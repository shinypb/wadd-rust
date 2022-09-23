// This implementation is all based on the documentary on ZDoom's wiki:
// https://zdoom.org/wiki/WAD

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek};

use fixedstr::fstr;

#[derive(Clone, Debug)]
pub struct DirectoryEntry {
    pub name: String,
    pub offset: i32,
    pub size: i32,
}

#[derive(Clone, Debug)]
pub struct LineDef {
    pub vertex_begin: i16,
    pub vertex_end: i16,
    pub flags: i16,
    pub line_type: i16,
    pub sector_tag: i16,
    pub sidedef_right: i16,
    pub sidedef_left: i16,
}

pub struct MapData {
    pub name: String,
    pub linedefs: Vec<LineDef>,
    pub sectors: Vec<Sector>,
    pub sidedefs: Vec<SideDef>,
    pub things: Vec<Thing>,
    pub vertexes: Vec<Vertex>,
}

#[derive(Clone, Debug)]
pub struct Sector {
    pub floor_height: i16,
    pub ceiling_height: i16,
    pub floor_texture: fstr<8>,
    pub ceiling_texture: fstr<8>,
    pub light_level: i16, // Vanilla Doom rounded the light level to the nearest multiple of 8, ZDoom shows unique light levels for all values
    pub special: u16,
    pub sector_tag: u16,
}

#[derive(Clone, Debug)]
pub struct SideDef {
    pub x: i16,
    pub y: i16,
    pub upper_texture: Option<fstr<8>>,
    pub lower_texture: Option<fstr<8>>,
    pub middle_texture: Option<fstr<8>>,
    pub sector: u16,
}

pub struct Thing {
    pub x: i16,
    pub y: i16,
    pub angle: i16,
    pub thing_type: i16,
    pub spawn_flags: i16,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    pub x: i16,
    pub y: i16,
}

pub struct Wad {
    pub directory: Vec<DirectoryEntry>,
    pub maps: Vec<MapData>,
    pub wad_type: WadType,
}

impl Wad {
    pub fn open(filename: &str) -> Result<Wad, String> {
        let mut file = File::open(filename).map_err(|err| err.to_string())?;

        let (wad_type, directory_offset, num_directory_entries) = decode_header(&mut file)?;
        let directory = decode_directory(&mut file, directory_offset, num_directory_entries)?;
        let maps = decode_maps(&mut file, &directory)?;

        Ok(Wad {
            directory,
            maps,
            wad_type,
        })
    }
}

#[derive(Debug)]
pub enum WadType {
    IWAD,
    PWAD,
}

//

fn decode_header(file: &mut File) -> Result<(WadType, i32, i32), String> {
    // https://zdoom.org/wiki/WAD#Header

    let mut header_buf = [0; 12];
    file.read_exact(&mut header_buf)
        .map_err(|err| err.to_string())?;

    let v = header_buf[0..4].to_vec();
    let wad_type = match String::from_utf8(v) {
        Ok(str) if str.eq("IWAD") => WadType::IWAD,
        Ok(str) if str.eq("PWAD") => WadType::PWAD,
        _ => {
            return Err(format!(
                "Invalid WAD; expected signature {:?} to be {:?} ('IWAD') or ({:?}) ('PWAD')",
                header_buf[0..4].to_vec(),
                String::from("IWAD").as_bytes(),
                String::from("PWAD").as_bytes()
            ))
        }
    };

    let num_directory_entries = i32::from_le_bytes(
        header_buf[4..8]
            .try_into()
            .map_err(|_| "Failed to get num_directory_entries bytes from buffer")?,
    );
    let directory_offset = i32::from_le_bytes(
        header_buf[8..12]
            .try_into()
            .map_err(|_| "Failed to get directory_offset bytes from buffer")?,
    );

    Ok((wad_type, directory_offset, num_directory_entries))
}

fn decode_directory(
    file: &mut File,
    offset: i32,
    num_entries: i32,
) -> Result<Vec<DirectoryEntry>, String> {
    // https://zdoom.org/wiki/WAD#Directory
    // The directory associates names of lumps with the data that belong to them. It
    // consists of a number of entries, each with a length of 16 bytes. The length of the
    // directory is determined by the number given in the WAD header.

    let mut entries: Vec<DirectoryEntry> = Vec::new();
    let mut entry_buf = [0; 16];
    for i in 0..num_entries {
        let entry_offset = offset + (i * 16);
        file.seek(std::io::SeekFrom::Start(entry_offset as u64))
            .map_err(|err| err.to_string())?;
        file.read_exact(&mut entry_buf)
            .map_err(|err| err.to_string())?;

        let lump_offset = entry_buf[0..4].try_into().map(i32::from_le_bytes).unwrap();
        let lump_size = entry_buf[4..8].try_into().map(i32::from_le_bytes).unwrap();
        let lump_name = buf_to_string(&entry_buf[8..16])
            .map_err(|_| format!("Lump {} at offset {} has invalid name", &i, &entry_offset))?;

        entries.push(DirectoryEntry {
            name: lump_name,
            offset: lump_offset,
            size: lump_size,
        })
    }

    Ok(entries)
}

fn buf_to_fstr<const N: usize>(input_buf: &[u8]) -> Result<fstr<N>, String> {
    match buf_to_string(input_buf) {
        Ok(str) => Ok(fstr::from(str)),
        Err(err) => Err(err.to_string()),
    }
}

fn buf_to_string(input_buf: &[u8]) -> Result<String, String> {
    // Strings in WADs are fixed length, end-padded with null characters as needed.
    let end_pos = input_buf
        .iter()
        .position(|c| *c == 0)
        .unwrap_or(input_buf.len());

    match String::from_utf8(input_buf[0..end_pos].to_vec()) {
        Ok(str) => Ok(str.trim_end_matches(char::is_control).to_owned()),
        Err(err) => Err(err.to_string()),
    }
}

fn decode_linedefs(file: &mut File, entry: &DirectoryEntry) -> Result<Vec<LineDef>, String> {
    const LINEDEF_SIZE: usize = std::mem::size_of::<LineDef>();
    assert!(entry.size % LINEDEF_SIZE as i32 == 0);

    file.seek(std::io::SeekFrom::Start(entry.offset as u64))
        .map_err(|err| err.to_string())?;

    let mut buf = [0; LINEDEF_SIZE];
    let mut linedefs = vec![];
    for _ in 0..(entry.size / LINEDEF_SIZE as i32) {
        file.read_exact(&mut buf).map_err(|err| err.to_string())?;

        let ints: Vec<i16> = buf
            .chunks_exact(2)
            .map(|c| c.try_into().map(i16::from_le_bytes).unwrap())
            .collect();

        linedefs.push(LineDef {
            vertex_begin: ints[0],
            vertex_end: ints[1],
            flags: ints[2],
            line_type: ints[3],
            sector_tag: ints[4],
            sidedef_right: ints[5],
            sidedef_left: ints[6],
        })
    }

    Ok(linedefs)
}

fn decode_sectors(file: &mut File, entry: &DirectoryEntry) -> Result<Vec<Sector>, String> {
    const SECTOR_SIZE: usize = 26; // can't use std::mem::size_of::<Sector>() because it has fstr's rather than 8-byte character arrays, as in the WAD
    assert!(entry.size % SECTOR_SIZE as i32 == 0);

    let _ = file.seek(std::io::SeekFrom::Start(entry.offset as u64));

    let mut buf = [0; SECTOR_SIZE];
    let mut sectors = vec![];
    for sector_id in 0..(entry.size / SECTOR_SIZE as i32) {
        file.read_exact(&mut buf).map_err(|err| err.to_string())?;

        sectors.push(Sector {
            floor_height: i16::from_le_bytes(buf[0..2].try_into().unwrap()),
            ceiling_height: i16::from_le_bytes(buf[2..4].try_into().unwrap()),
            floor_texture: buf_to_fstr(&buf[4..12])
                .map_err(|_| format!("Sector {} has invalid floor texture", sector_id))?,
            ceiling_texture: buf_to_fstr(&buf[12..20])
                .map_err(|_| format!("Sector {} has invalid ceiling texture", sector_id))?,
            light_level: i16::from_le_bytes(buf[20..22].try_into().unwrap()),
            special: u16::from_le_bytes(buf[22..24].try_into().unwrap()),
            sector_tag: u16::from_le_bytes(buf[24..26].try_into().unwrap()),
        })
    }

    Ok(sectors)
}

fn decode_sidedefs(file: &mut File, entry: &DirectoryEntry) -> Result<Vec<SideDef>, String> {
    const SIDEDEF_SIZE: usize = 30; // can't use std::mem::size_of::<SideDef>() because it has fstr's rather than 8-byte character arrays, as in the WAD
    assert!(entry.size % SIDEDEF_SIZE as i32 == 0);

    file.seek(std::io::SeekFrom::Start(entry.offset as u64))
        .map_err(|err| err.to_string())?;

    let mut buf = [0; SIDEDEF_SIZE];
    let mut sidedefs = vec![];
    for _ in 0..(entry.size / SIDEDEF_SIZE as i32) {
        file.read_exact(&mut buf).map_err(|err| err.to_string())?;

        const NO_TEXTURE_PLACEHOLDER: &str = "-";
        let upper_texture =
            Some(buf_to_fstr(&buf[4..12])?).filter(|str| str != NO_TEXTURE_PLACEHOLDER);
        let lower_texture =
            Some(buf_to_fstr(&buf[12..20])?).filter(|str| str != NO_TEXTURE_PLACEHOLDER);
        let middle_texture =
            Some(buf_to_fstr(&buf[20..28])?).filter(|str| str != NO_TEXTURE_PLACEHOLDER);

        sidedefs.push(SideDef {
            x: i16::from_le_bytes(buf[0..2].try_into().unwrap()),
            y: i16::from_le_bytes(buf[2..4].try_into().unwrap()),
            upper_texture,
            lower_texture,
            middle_texture,
            sector: u16::from_le_bytes(buf[28..30].try_into().unwrap()),
        })
    }
    Ok(sidedefs)
}

fn decode_things(file: &mut File, entry: &DirectoryEntry) -> Result<Vec<Thing>, String> {
    const THING_SIZE: usize = std::mem::size_of::<Thing>();
    assert!(entry.size % THING_SIZE as i32 == 0);

    file.seek(std::io::SeekFrom::Start(entry.offset as u64))
        .map_err(|err| err.to_string())?;

    let mut buf = [0; THING_SIZE];
    let mut things = vec![];
    for _ in 0..(entry.size / THING_SIZE as i32) {
        file.read_exact(&mut buf).map_err(|err| err.to_string())?;

        let ints: Vec<i16> = buf
            .chunks_exact(2)
            .map(|c| c.try_into().map(i16::from_le_bytes).unwrap())
            .collect();

        things.push(Thing {
            x: ints[0],
            y: ints[1],
            angle: ints[2],
            thing_type: ints[3],
            spawn_flags: ints[4],
        })
    }
    Ok(things)
}

fn decode_vertexes(file: &mut File, entry: &DirectoryEntry) -> Result<Vec<Vertex>, String> {
    const VERTEX_SIZE: usize = std::mem::size_of::<Vertex>();
    assert!(entry.size % VERTEX_SIZE as i32 == 0);

    file.seek(std::io::SeekFrom::Start(entry.offset as u64))
        .map_err(|err| err.to_string())?;

    let mut buf = [0; VERTEX_SIZE];
    let mut vertexes = vec![];
    for _ in 0..(entry.size / VERTEX_SIZE as i32) {
        file.read_exact(&mut buf).map_err(|err| err.to_string())?;

        vertexes.push(Vertex {
            x: i16::from_le_bytes(buf[0..2].try_into().unwrap()),
            y: i16::from_le_bytes(buf[2..4].try_into().unwrap()),
        })
    }
    Ok(vertexes)
}

fn decode_lumps<T>(
    file: &mut File,
    lumps_map: &HashMap<String, DirectoryEntry>,
    lump_type: &str,
    decoder_fn: fn(&mut File, &DirectoryEntry) -> Result<Vec<T>, String>,
) -> Result<Vec<T>, String> {
    let lumps = lumps_map
        .get(lump_type)
        .ok_or(format!("No {} lumps found", lump_type))?;
    println!("lump type {} lumps {:?}", lump_type, lumps);
    Ok(decoder_fn(file, lumps)?)
}

fn decode_maps(file: &mut File, directory: &Vec<DirectoryEntry>) -> Result<Vec<MapData>, String> {
    let map_lump_names: Vec<String> = vec![
        "BLOCKMAP", "LINEDEFS", "NODES", "REJECT", "SCRIPTS", "SECTORS", "SEGS", "SIDEDEFS",
        "SSECTORS", "THINGS", "VERTEXES",
    ]
    .iter()
    .map(|str| str.to_string())
    .collect();

    // Collect all of the lumps on a per-map basis
    // TODO: I don't love this code; revisit it
    let mut map_lumps: HashMap<String, HashMap<String, DirectoryEntry>> = HashMap::new();
    for mut i in 0..directory.len() {
        let d = directory.get(i).unwrap();
        if d.size == 0 && d.offset > 0 {
            // this lump is the start of a map
            let map_name = d.name.clone();
            let mut lumps = HashMap::new();

            loop {
                i += 1;
                let d = directory.get(i).unwrap();
                if !map_lump_names.contains(&d.name) {
                    break;
                }
                lumps.insert(d.name.to_string(), d.clone());
            }

            map_lumps.insert(map_name, lumps);
        }
    }

    // Create MapData instances based on the lumps
    let mut maps: Vec<MapData> = vec![];
    for (map_name, lumps) in &map_lumps {
        let linedefs = decode_lumps(file, lumps, "LINEDEFS", decode_linedefs)?;
        let things = decode_lumps(file, lumps, "THINGS", decode_things)?;
        let vertexes = decode_lumps(file, lumps, "VERTEXES", decode_vertexes)?;
        let sidedefs = decode_lumps(file, lumps, "SIDEDEFS", decode_sidedefs)?;
        let sectors = decode_lumps(file, lumps, "SECTORS", decode_sectors)?;

        maps.push(MapData {
            name: map_name.to_string(),
            linedefs,
            sectors,
            sidedefs,
            things,
            vertexes,
        })
    }

    maps.sort_by_key(|map| map.name.clone());

    return Ok(maps);
}
