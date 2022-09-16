// This implementation is all based on the documentary on ZDoom's wiki:
// https://zdoom.org/wiki/WAD

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek};

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
    pub floor_texture: [u8; 8],
    pub ceiling_texture: [u8; 8],
    pub light_level: i16, // Vanilla Doom rounded the light level to the nearest multiple of 8, ZDoom shows unique light levels for all values
    pub special: u16,
    pub sector_tag: u16,
}

#[derive(Clone, Debug)]
pub struct SideDef {
    pub x: i16,
    pub y: i16,
    pub upper_texture: [u8; 8],
    pub lower_texture: [u8; 8],
    pub middle_texture: [u8; 8],
    pub sector: u16,
}

pub struct Thing {
  pub x: i16,
  pub y: i16,
  pub angle: i16,
  pub thing_type: i16,
  pub spawn_flags: i16,
}

enum ThingType {
  Player1Start = 1,
  Player2Start = 2,
  Player3Start = 3,
  Player4Start = 4,
  BlueCard = 5,
  YellowCard = 6,
  SpiderMastermind = 7,
  Backpack = 8,
  ShotgunGuy = 9,
  GibbedMarine = 10,
  DeathmatchStart = 11,
  GibbedMarineExtra = 12,
  RedCard = 13,
  DeadMarine = 15,
  Cyberdemon = 16,
  CellPack = 17,
  DeadZombieMan = 18,
  DeadShotgunGuy = 19,
  DeadDoomImp = 20,
  DeadDemon = 21,
  DeadCacodemon = 22,
  DeadLostSoul = 23,
  Gibs = 24,
  DeadStick = 25,
  LiveStick = 26,
  HeadOnAstick = 27,
  HeadsOnAstick = 28,
  HeadCandles = 29,
  TallGreenColumn = 30,
  ShortGreenColumn = 31,
  TallRedColumn = 32,
  ShortRedColumn = 33,
  Candlestick = 34,
  Candelabra = 35,
  HeartColumn = 36,
  SkullColumn = 37,
  RedSkull = 38,
  YellowSkull = 39,
  BlueSkull = 40,
  EvilEye = 41,
  FloatingSkull = 42,
  TorchTree = 43,
  BlueTorch = 44,
  GreenTorch = 45,
  RedTorch = 46,
  Stalagtite = 47,
  TechPillar = 48,
  BloodyTwitch = 49,
  Meat2 = 50,
  Meat3 = 51,
  Meat4 = 52,
  Meat5 = 53,
  BigTree = 54,
  ShortBlueTorch = 55,
  ShortGreenTorch = 56,
  ShortRedTorch = 57,
  Spectre = 58,
  NonsolidMeat2 = 59,
  NonsolidMeat4 = 60,
  NonsolidMeat3 = 61,
  NonsolidMeat5 = 62,
  NonsolidTwitch = 63,
  Archvile = 64, // Doom 2
  ChaingunGuy = 65, // Doom 2
  Revenant = 66, // Doom 2
  Fatso = 67, // Doom 2
  Arachnotron = 68, // Doom 2
  HellKnight = 69, // Doom 2
  BurningBarrel = 70, // Doom 2
  PainElemental = 71, // Doom 2
  CommanderKeen = 72, // Doom 2
  HangNoGuts = 73, // Doom 2
  HangBnoBrain = 74, // Doom 2
  HangTlookingDown = 75, // Doom 2
  HangTskull = 76, // Doom 2
  HangTlookingUp = 77, // Doom 2
  HangTnoBrain = 78, // Doom 2
  ColonGibs = 79, // Doom 2
  SmallBloodPool = 80, // Doom 2
  BrainStem = 81, // Doom 2
  SuperShotgun = 82, // Doom 2
  Megasphere = 83, // Doom 2
  WolfensteinSs = 84, // Doom 2
  TechLamp = 85, // Doom 2
  TechLamp2 = 86, // Doom 2
  BossTarget = 87, // Doom 2
  BossBrain = 88, // Doom 2
  BossEye = 89, // Doom 2
  Zbridge = 118,
  Shotgun = 2001,
  Chaingun = 2002,
  RocketLauncher = 2003,
  PlasmaRifle = 2004,
  Chainsaw = 2005,
  Bfg9000 = 2006,
  Clip = 2007,
  Shell = 2008,
  RocketAmmo = 2010,
  StimPack = 2011,
  MediKit = 2012,
  SoulSphere = 2013,
  HealthBonus = 2014,
  ArmorBonus = 2015,
  GreenArmor = 2018,
  BlueArmor = 2019,
  InvulnerabilitySphere = 2022,
  Berserk = 2023,
  BlurSphere = 2024,
  RadSuit = 2025,
  AllMap = 2026,
  Column = 2028,
  ExplosiveBarrel = 2035,
  Infrared = 2045,
  RocketBox = 2046,
  Cell = 2047,
  ClipBox = 2048,
  ShellBox = 2049,
  DoomImp = 3001,
  Demon = 3002,
  BaronOfHell = 3003,
  ZombieMan = 3004,
  Cacodemon = 3005,
  LostSoul = 3006,
  Pistol = 5010,
  Stalagmite = 5050,
  StealthArachnotron = 9050, // Doom 2
  StealthArchvile = 9051, // Doom 2
  StealthBaron = 9052,
  StealthCacodemon = 9053,
  StealthChaingunGuy = 9054, // Doom 2
  StealthDemon = 9055,
  StealthHellKnight = 9056, // Doom 2
  StealthDoomImp = 9057,
  StealthFatso = 9058, // Doom 2
  StealthRevenant = 9059, // Doom 2
  StealthShotgunGuy = 9060,
  StealthZombieMan = 9061,
  ScriptedMarine = 9100,
  MarineFist = 9101,
  MarineBerserk = 9102,
  MarineChainsaw = 9103,
  MarinePistol = 9104,
  MarineShotgun = 9105,
  MarineSsg = 9106,
  MarineChaingun = 9107,
  MarineRocket = 9108,
  MarinePlasma = 9109,
  MarineRailgun = 9110,
  MarineBfg = 9111,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    pub x: i16,
    pub y: i16,
}

pub struct Wad {
  pub directory: Vec<DirectoryEntry>,
  pub maps: Vec<MapData>,
  pub wad_type: WadType
}

impl Wad {
  pub fn open(filename: &str) -> Wad {
      let mut file = match File::open(filename) {
          Ok(file) => file,
          Err(error) => panic!("Failed to open {}: {}", filename, error),
      };

      let (wad_type, directory_offset, num_directory_entries) = decode_header(&mut file);
      let directory = decode_directory(&mut file, directory_offset, num_directory_entries);
      let maps = decode_maps(&mut file, &directory);

      Wad {
          directory,
          maps,
          wad_type,
      }
  }
}

#[derive(Debug)]
pub enum WadType {
    IWAD,
    PWAD
}

//

fn decode_header(file: &mut File) -> (WadType, i32, i32) {
  // https://zdoom.org/wiki/WAD#Header

  let mut header_buf = [0; 12];
  let _ = file.read_exact(&mut header_buf);

  let v = header_buf[0..4].to_vec();
  let wad_type = match String::from_utf8(v) {
      Ok(str) if str.eq("IWAD") => WadType::IWAD,
      Ok(str) if str.eq("PWAD") => WadType::PWAD,
      _ => panic!("Invalid WAD; expected signature {:?} to be 'IWAD' ({:?}) or 'PWAD' ({:?})",
          header_buf[0..4].to_vec(),
          String::from("IWAD").as_bytes(),
          String::from("PWAD").as_bytes()
      )
  };

  let num_directory_entries = i32::from_le_bytes(header_buf[4..8].try_into().expect("Failed to get bytes from buffer"));
  let directory_offset = i32::from_le_bytes(header_buf[8..12].try_into().expect("Failed to get bytes from buffer"));

  (wad_type, directory_offset, num_directory_entries)
}

fn decode_directory(file: &mut File, offset: i32, num_entries: i32) -> Vec<DirectoryEntry> {
  // https://zdoom.org/wiki/WAD#Directory
  // The directory associates names of lumps with the data that belong to them. It
  // consists of a number of entries, each with a length of 16 bytes. The length of the
  // directory is determined by the number given in the WAD header.

  let mut entries: Vec<DirectoryEntry> = Vec::new();
  let mut entry_buf = [0; 16];
  for i in 0..num_entries {
      let entry_offset = offset + (i * 16);
      let _ = file.seek(std::io::SeekFrom::Start(entry_offset as u64));
      let _ = file.read_exact(&mut entry_buf);

      let lump_offset = i32::from_le_bytes(entry_buf[0..4].try_into().expect("Failed to get bytes from buffer"));
      let lump_size = i32::from_le_bytes(entry_buf[4..8].try_into().expect("Failed to get bytes from buffer"));
      let lump_name = buf_to_string(entry_buf[8..16].to_vec());

      entries.push(DirectoryEntry {
          name: lump_name,
          offset: lump_offset,
          size: lump_size,
      })
  }

  entries
}

fn buf_to_string(input_buf: Vec<u8>) -> String {
  match String::from_utf8(input_buf.to_vec()) {
      Ok(str) => str.trim_end_matches(char::is_control).to_owned(), // wad strings are fixed length, end-padded with nulls
      _ => panic!("Failed to parse string from input {:?}", &input_buf)
  }
}

fn decode_linedefs(file: &mut File, entry: &DirectoryEntry) -> Vec<LineDef> {
  const LINEDEF_SIZE: usize = std::mem::size_of::<LineDef>();
  assert!(entry.size % LINEDEF_SIZE as i32 == 0);

  let _ = file.seek(std::io::SeekFrom::Start(entry.offset as u64));

  let mut buf = [0; LINEDEF_SIZE];
  return (0..(entry.size / LINEDEF_SIZE as i32)).map(|_| {
      let _ = file.read_exact(&mut buf);

      LineDef {
          vertex_begin:  i16::from_le_bytes(buf[0..2].try_into().unwrap()),
          vertex_end:    i16::from_le_bytes(buf[2..4].try_into().unwrap()),
          flags:         i16::from_le_bytes(buf[4..6].try_into().unwrap()),
          line_type:     i16::from_le_bytes(buf[6..8].try_into().unwrap()),
          sector_tag:    i16::from_le_bytes(buf[8..10].try_into().unwrap()),
          sidedef_right: i16::from_le_bytes(buf[10..12].try_into().unwrap()),
          sidedef_left:  i16::from_le_bytes(buf[12..14].try_into().unwrap()),
      }
  }).collect();
}

fn decode_sectors(file: &mut File, entry: &DirectoryEntry) -> Vec<Sector> {
  const SECTOR_SIZE: usize = std::mem::size_of::<Sector>();
  assert!(entry.size % SECTOR_SIZE as i32 == 0);

  let _ = file.seek(std::io::SeekFrom::Start(entry.offset as u64));

  let mut buf = [0; SECTOR_SIZE];
  return (0..(entry.size / SECTOR_SIZE as i32)).map(|_| {
      let _ = file.read_exact(&mut buf);

      Sector {
          floor_height:    i16::from_le_bytes(buf[0..2].try_into().unwrap()),
          ceiling_height:  i16::from_le_bytes(buf[2..4].try_into().unwrap()),
          floor_texture:   buf[4..12].try_into().unwrap(),
          ceiling_texture: buf[12..20].try_into().unwrap(),
          light_level:     i16::from_le_bytes(buf[20..22].try_into().unwrap()),
          special:         u16::from_le_bytes(buf[22..24].try_into().unwrap()),
          sector_tag:             u16::from_le_bytes(buf[24..26].try_into().unwrap()),
      }
  }).collect();
}

fn decode_sidedefs(file: &mut File, entry: &DirectoryEntry) -> Vec<SideDef> {
  const SIDEDEF_SIZE: usize = std::mem::size_of::<SideDef>();
  assert!(entry.size % SIDEDEF_SIZE as i32 == 0);

  let _ = file.seek(std::io::SeekFrom::Start(entry.offset as u64));

  fn buf_to_array(buf: &[u8]) -> [u8; 8] {
      assert!(buf.len() == 8);
      let mut array = [0 as u8; 8];
      array.copy_from_slice(buf);
      return array;
  }

  let mut buf = [0; SIDEDEF_SIZE];
  return (0..(entry.size / SIDEDEF_SIZE as i32)).map(|_| {
      let _ = file.read_exact(&mut buf);

      SideDef {
          x: i16::from_le_bytes(buf[0..2].try_into().unwrap()),
          y: i16::from_le_bytes(buf[2..4].try_into().unwrap()),
          upper_texture: buf[4..12].try_into().unwrap(),
          lower_texture: buf[12..20].try_into().unwrap(),
          middle_texture: buf[20..28].try_into().unwrap(),
          sector: u16::from_le_bytes(buf[28..30].try_into().unwrap()),
      }
  }).collect();
}

fn decode_things(file: &mut File, entry: &DirectoryEntry) -> Vec<Thing> {
  const THING_SIZE: usize = std::mem::size_of::<Thing>();
  assert!(entry.size % THING_SIZE as i32 == 0);

  let _ = file.seek(std::io::SeekFrom::Start(entry.offset as u64));

  let mut buf = [0; THING_SIZE];
  return (0..(entry.size / THING_SIZE as i32)).map(|_| {
      let _ = file.read_exact(&mut buf);
      Thing {
          x: i16::from_le_bytes(buf[0..2].try_into().unwrap()),
          y: i16::from_le_bytes(buf[2..4].try_into().unwrap()),
          angle: i16::from_le_bytes(buf[4..6].try_into().unwrap()),
          thing_type: i16::from_le_bytes(buf[6..8].try_into().unwrap()),
          spawn_flags: i16::from_le_bytes(buf[8..10].try_into().unwrap()),
      }
  }).collect();
}

fn decode_vertexes(file: &mut File, entry: &DirectoryEntry) -> Vec<Vertex> {
  const VERTEX_SIZE: usize = std::mem::size_of::<Vertex>();
  assert!(entry.size % VERTEX_SIZE as i32 == 0);

  let _ = file.seek(std::io::SeekFrom::Start(entry.offset as u64));

  let mut buf = [0; VERTEX_SIZE];
  return (0..(entry.size / VERTEX_SIZE as i32)).map(|_| {
      let _ = file.read_exact(&mut buf);
      Vertex {
          x: i16::from_le_bytes(buf[0..2].try_into().unwrap()),
          y: i16::from_le_bytes(buf[2..4].try_into().unwrap()),
      }
  }).collect();
}

fn decode_lumps<T>(file: &mut File, lumps: &HashMap<String, DirectoryEntry>, lump_type: &str, decoder_fn: fn(&mut File, &DirectoryEntry) -> Vec<T>) -> Vec<T> {
  lumps
      .get(lump_type)
      .map(|d| { decoder_fn(file, d) })
      .unwrap_or(vec!())
}

fn decode_maps(file: &mut File, directory: &Vec<DirectoryEntry>) -> Vec<MapData> {
  let map_lump_names = vec!(
      String::from("BLOCKMAP"),
      String::from("LINEDEFS"),
      String::from("NODES"),
      String::from("REJECT"),
      String::from("SCRIPTS"),
      String::from("SECTORS"),
      String::from("SEGS"),
      String::from("SIDEDEFS"),
      String::from("SSECTORS"),
      String::from("THINGS"),
      String::from("VERTEXES"),
  );

  // Collect all of the lumps on a per-map basis
  let mut map_lumps: HashMap<String, HashMap<String, DirectoryEntry>> = HashMap::new();
  for mut i in 0..directory.len() {
      let d = directory.get(i).unwrap();
      if d.size == 0 && d.offset > 0 { // this lump is the start of a map
          let map_name = String::from(&d.name);
          let mut lumps = HashMap::new();

          loop {
              i += 1;
              let d = directory.get(i).unwrap();
              if !map_lump_names.contains(&d.name) {
                  break
              }
              lumps.insert(d.name.to_string(), d.clone());
          }

          map_lumps.insert(map_name, lumps);
      }
  }

  // Create MapData instances based on the lumps
  let mut maps: Vec<MapData> = map_lumps.iter().map(|(map_name, lumps)| {
      let linedefs = decode_lumps(file, lumps, &String::from("LINEDEFS"), decode_linedefs);
      let things = decode_lumps(file, lumps, &String::from("THINGS"), decode_things);
      let vertexes = decode_lumps(file, lumps, &String::from("VERTEXES"), decode_vertexes);
      let sidedefs = decode_lumps(file, lumps, &String::from("SIDEDEFS"), decode_sidedefs);
      let sectors = decode_lumps(file, lumps, &String::from("SECTORS"), decode_sectors);

      MapData {
          name: map_name.to_string(),
          linedefs,
          sectors,
          sidedefs,
          things,
          vertexes,
      }
  }).collect();

  maps.sort_by_key(|map| map.name.clone());

  return maps;
}
