#![allow(dead_code)]
#![allow(unused_variables)]

use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{Read, Seek, Write};
use std::process::exit;

use svg::Document;
use svg::node::element::{Path, Line};
use svg::node::element::path::Data;

fn main() {
    let args: Vec<String> = env::args().collect();
    let params = &args[1..];

    match args.as_slice() {
        [_, filename] => handle_command(filename, &String::from("info"), &[]),
        [_, filename, command, rest @ ..] => handle_command(filename, command, rest),
        _ => print_usage_and_exit()
    }
}

fn handle_command(filename: &String, command: &str, params: &[String]) {
    let wad = Wad::open(filename);

    match command {
        "info" => show_info(&wad),
        "maps" => list_maps(&wad),
        "svg" => {
            match params.first() {
                Some(map_name) => extract_map(&wad, &map_name),
                None => {
                    println!("Dumping all maps...");
                    for map in &wad.maps {
                        extract_map(&wad, &map.name)
                    }
                }
            }
        }
        _ => {
            println!("Sorry, I don't know how to {}.", command);
            std::process::exit(1);
        }
    }
}

fn extract_map(wad: &Wad, map_name: &str) {
    let map = wad.maps.iter().find(|map| map.name == map_name).expect("That map does not exist.");

    let lines: Vec<(Vertex, Vertex)> = map.linedefs.iter().map(|linedef| {
        let v1 = map.vertexes[linedef.vertex_begin as usize];
        let v2 = map.vertexes[linedef.vertex_end as usize];
        
        (v1, v2)
    }).collect();

    assert!(lines.len() > 0);

    // Figure out which offsets to use to put the map in the top left corner
    let min_x: i16 = lines
        .iter()
        .map(|line| line.0.x.min(line.1.x))
        .min()
        .unwrap();
    let max_x: i16 = lines
        .iter()
        .map(|line| line.0.x.max(line.1.x))
        .max()
        .unwrap();
    let min_y: i16 = lines
        .iter()
        .map(|line| line.0.y.min(line.1.y))
        .min()
        .unwrap();
    let max_y: i16 = lines
        .iter()
        .map(|line| line.0.y.max(line.1.y))
        .max()
        .unwrap();

    let offset_x = 0 - min_x;
    let offset_y = 0 - min_y;
    let height = max_y - min_y;
    let width = max_x - min_x;

    // Get the sectors
    #[derive(Debug)]
    struct RenderableSector {
        linedefs: Vec<LineDef>,
        lines: Vec<(Vertex, Vertex, LineDef)>,
        sector: Sector,
    }
    let sectors: Vec<RenderableSector> = map.sectors.iter().enumerate().map(|(sector_index, sector)| {
        let sector_lines: Vec<LineDef> = map
            .linedefs
            .iter()
            .enumerate()
            .filter(|(line_index, linedef)| {
                if linedef.sidedef_right >= 0 {
                    let sidedef_right = map.sidedefs[linedef.sidedef_right as usize].clone();
                    if (sidedef_right.sector as usize) == sector_index {
                        return true
                    }
                }
                if linedef.sidedef_left >= 0 {
                    let sidedef_left = map.sidedefs[linedef.sidedef_left as usize].clone();
                    if (sidedef_left.sector as usize) == sector_index {
                        return true
                    }
                }
                false
            })
            .map(|(line_index, linedef)| linedef.clone())
            .collect();
        let vertex_lines: Vec<(Vertex, Vertex, LineDef)> = sector_lines
            .iter()
            .map(|linedef| {
                let v1 = map.vertexes[linedef.vertex_begin as usize];
                let v2 = map.vertexes[linedef.vertex_end as usize];

                // Vertexes are stored upside down from what we'd expect, so flip their y coordinate
                let v1 = Vertex {
                    y: max_y - (v1.y - min_y),
                    ..v1
                };
                let v2 = Vertex {
                    y: max_y - (v2.y - min_y),
                    ..v2
                };

                (v1, v2, linedef.clone())
            })
            .collect();
        RenderableSector {
            linedefs: sector_lines,
            lines: vertex_lines,
            sector: sector.clone(),
        }
    }).collect();

    // Write it out
    println!("{} has {} sectors and is {}x{}", &map_name, sectors.len(), width, height);
    let mut doc = Document::new()
        .set("viewBox", format!("0 0 {} {}", width, height));

    let mut sector_id = -1;
    for sector in sectors.iter() {
        sector_id += 1;
        println!("\nSector {} has {} lines:", sector_id, sector.lines.len());
        for (from_v, to_v, _) in sector.lines.clone() {
            println!("({}, {}) -> ({}, {})",
                from_v.x + offset_x, from_v.y + offset_y,
                to_v.x + offset_x, to_v.y + offset_y
            );
        }

        let mut data = Data::new();
        let mut pending_lines = sector.lines.clone();
        while !pending_lines.is_empty() {
            // Sectors consist of a series of lines that may or may not all connect with each other:
            // a sector might just be a basic polygon, but it could also have a donut-like shape with
            // an empty spot or another sector contained within it. Because of this, we can't just
            // just iterate over the lines in the sector and add them to a single Path. Instead, we
            // create a Path, pop the next line off the list, and walk through all of the remaining
            // lines until we close the path. We continue this until all lines have been added to a
            // path.
            // We need at least 2 lines total to draw a triangle, the simplest possible shape:
            // a line from "A" to "B" and a line from "B" to "C". The line back from "C" to "A" can
            // be implicit.
            if pending_lines.len() < 2 {
                println!("WARNING: Sector {} only has {} lines left", sector_id, pending_lines.len());
            }

            let (initial_v, mut to_v, _) = pending_lines.remove(0);

            data = data
                .move_to((initial_v.x + offset_x, initial_v.y + offset_y))
                .line_to((to_v.x + offset_x, to_v.y + offset_y));

            while let Some(next_line_idx) = pending_lines.iter().position(|(other_from_v, other_to_v, _)| {
                // Look for any other lines that share our `to_v` vertex, regardless of direction
                other_from_v == &to_v
                || other_to_v == &to_v
            }) {
                let prev_v = to_v;
                let (next_from_v, next_to_v, _) = pending_lines.remove(next_line_idx);
                if next_to_v == to_v {
                    // This line points in the opposite direction that we want, so swap from_v <-> to_v
                    to_v = next_from_v
                } else {
                    to_v = next_to_v
                }

                data = data.line_to((to_v.x + offset_x, to_v.y + offset_y));
            }
            data = data.close();
        }

        let light_level = sector.sector.light_level.clamp(0, 255);
        let fill_color = format!("rgb({}, {}, {})", light_level, light_level, light_level);
        // TODO: eventually it'd be nice to have each individual line be colored differently depending
        // on whether it was one-sided or two-sided. Doing this would require drawing the path without
        // a stroke, and drawing all of the lines individually.
        let path = Path::new()
            .set("id", format!("sector{}", sector_id))
            .set("fill", fill_color)
            .set("stroke", "none")
            .set("d", data);

        doc = doc.add(path);
    }

    // Draw sector lines
    for sector in sectors.iter() {
        for (from_v, to_v, linedef) in sector.lines.iter() {
            let mut line = Line::new()
                .set("x1", from_v.x + offset_x)
                .set("y1", from_v.y + offset_y)
                .set("x2", to_v.x + offset_x)
                .set("y2", to_v.y + offset_y);
            if linedef.sidedef_left < 0 || linedef.sidedef_right < 0 {
                // one-sided line
                line = line.set("stroke", "red")
                    .set("stroke-width", "2");
            } else {
                // two-sided line
                line = line.set("stroke", "rgba(255, 0, 0, 0.25)")
                    .set("stroke-width", "1");
            }
            doc = doc.add(line);
        }
    }
    for line in sectors[0].lines.clone() {
        println!("{:?}", line);
    }

    // Save as SVG
    svg::save(format!("{}.svg", &map_name), &doc).unwrap();

    // Save as HTML
    let html = format!(r#"<!DOCTYPE html>
<html lang="en-US">
<head>
    <meta charset="utf-8">
    <style>
        html {{
            background: white;
            color: white;
            /* https://stackoverflow.com/posts/35362074/revisions */
            background-image: linear-gradient(45deg, #ccc 25%, transparent 25%), linear-gradient(-45deg, #ccc 25%, transparent 25%), linear-gradient(45deg, transparent 75%, #ccc 75%), linear-gradient(-45deg, transparent 75%, #ccc 75%);
            background-size: 20px 20px;
            background-position: 0 0, 0 10px, 10px -10px, -10px 0px;
        }}
    </style>
    <meta name="viewport" content="width=device-width, initial-scale=1.0, viewport-fit=cover">
</head>
<body>
    {}
</body>
"#, doc.to_string());

    let filename = format!("{}.html", &map_name);
    let mut output = File::create(&filename).expect("Failed to create file");
    match write!(output, "{}", html) {
        Ok(_) => (),
        Err(error) => panic!("Write failed: {}", error),
    }

}

fn list_maps(wad: &Wad) {
    println!("{} maps:", wad.maps.len());
    for map in &wad.maps {
        println!("- {} ({} linedefs, {} sectors, {} things, {} vertexes)", map.name, map.linedefs.len(), map.sectors.len(), map.things.len(), map.vertexes.len());
    }
}

fn show_info(wad: &Wad) {
    let wad_type = match wad.wad_type {
        WadType::IWAD => "IWAD",
        WadType::PWAD => "PWAD",
    };
    println!("{} with {} lumps in its directory:", &wad_type, wad.directory.len());
    for d in &wad.directory {
        if d.size > 0 {
            println!("- {: <8}\t{} bytes starting at {}", d.name, d.size, d.offset);
        } else {
            println!("- {: <8}\tempty lump", d.name);
        }
    }

}

fn print_usage_and_exit() {
    let args: Vec<String> = env::args().collect();
    let executable = args.first().unwrap();
    println!("usage: {} /path/to/a/doom.wad [command]", executable);
    println!("\nAvailable commands:");
    println!("- info");
    println!("  prints info about the WAD. This is the default if a command is not specified.");
    println!("- maps");
    println!("  prints a list of the maps in the WAD.");
    println!("- svg [map name]");
    println!("  extracts the given map to an SVG file in the current directory with the filename [map name].svg");
    println!("  (if no map name is specified, every map in the WAD will be extracted automatically)");

    exit(255);
}

#[derive(Debug)]
enum WadType {
    IWAD,
    PWAD
}

enum Things {
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

#[derive(Clone, Debug)]
struct DirectoryEntry {
    name: String,
    offset: i32,
    size: i32,
}

struct Wad {
    directory: Vec<DirectoryEntry>,
    maps: Vec<MapData>,
    wad_type: WadType
}

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
#[derive(Clone, Debug)]
struct LineDef {
    vertex_begin: i16,
    vertex_end: i16,
    flags: i16,
    line_type: i16,
    sector_tag: i16,
    sidedef_right: i16,
    sidedef_left: i16,
}

#[derive(Clone, Debug)]
struct Sector {
    floor_height: i16,
    ceiling_height: i16,
    floor_texture: [u8; 8],
    ceiling_texture: [u8; 8],
    light_level: i16, // Vanilla Doom rounded the light level to the nearest multiple of 8, ZDoom shows unique light levels for all values
    special: u16,
    sector_tag: u16,
}

#[derive(Clone, Debug)]
struct SideDef {
    x: i16,
    y: i16,
    upper_texture: [u8; 8],
    lower_texture: [u8; 8],
    middle_texture: [u8; 8],
    sector: u16,
}


#[derive(Clone, Copy, Debug, PartialEq)]
struct Vertex {
    x: i16,
    y: i16,
}
struct Thing {
    x: i16,
    y: i16,
    angle: i16,
    thing_type: i16,
    spawn_flags: i16,
}

struct MapData {
    name: String,
    linedefs: Vec<LineDef>,
    sectors: Vec<Sector>,
    sidedefs: Vec<SideDef>,
    things: Vec<Thing>,
    vertexes: Vec<Vertex>,
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

impl Wad {
    fn open(filename: &str) -> Wad {
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
