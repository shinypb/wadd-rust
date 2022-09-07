#![allow(dead_code)]
#![allow(unused_variables)]

use std::env;
use std::fs::File;
use std::io::{Read, Seek};

fn main() {
    let args: Vec<String> = env::args().collect();
    let params = &args[1..];

    match args.as_slice() {
        [_, filename] => handle_command(filename, &String::from("info"), &[]),
        [_, filename, command, rest @ ..] => handle_command(filename, command, rest),
        _ => print_usage_and_exit(&args[0])
    }
}

fn handle_command(filename: &String, command: &str, params: &[String]) {
    println!("Filename: {}, Command: {}, Params: {:?}", filename, command, params);

    let wad = Wad::open(filename);

    match command {
        "list-maps" => list_maps(&wad),
        "info" => show_info(&wad),
        _ => {
            println!("Sorry, I don't know how to {}", command);
            std::process::exit(1);
        }
    }
}

fn list_maps(_wad: &Wad) {
}

fn show_info(_wad: &Wad) {
}

fn print_usage_and_exit(executable: &String) {
    println!("usage: {} /path/to/a/doom.wad command", executable);
}

enum MapLumpType {
    Blockmap,
    LineDefs,
    Nodes,
    Reject,
    Scripts,
    Sectors,
    Segs,
    SideDefs,
    SSectors,
    Things,
    Vertexes,
}

#[derive(Debug)]
enum WadType {
    IWAD,
    PWAD
}

#[derive(Debug)]
struct DirectoryEntry {
    name: String,
    offset: i32,
    size: i32,
}

struct Wad {
    directory: Vec<DirectoryEntry>,
    wad_type: WadType
}

fn decode_header(file: &mut File) -> (WadType, i32, i32) {
    // https://zdoom.org/wiki/WAD#Header

    let mut header_buf = [0; 12];
    let _ = file.read_exact(&mut header_buf);
    println!("Header {:?}", header_buf);

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

        println!("Entry #{}: {}, {} bytes starting at {}", i, lump_name, lump_size, lump_offset);

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

struct LineDef {
    vertex_begin: i16,
    vertex_end: i16,
    flags: i16,
    line_type: i16,
    sector_tag: i16,
    sidedef_right: i16,
    sidedef_left: i16,
}
struct Vertex {

}
struct Thing {
}

struct MapData {
    name: String,
    lumps: Vec<DirectoryEntry>,
    linedefs: Vec<LineDef>,
}

fn decode_linedefs(file: &mut File, entry: &DirectoryEntry) -> Vec<LineDef> {
    let linedef_size = 14; // 7 x i16 per linedef
    assert!(entry.size % linedef_size == 0); // 7 16-bit ints

    let _ = file.seek(std::io::SeekFrom::Start(entry.offset as u64));

    let mut linedefs: Vec<LineDef> = vec!();
    let mut buf = [0; 14];
    for i in 0..(entry.size / linedef_size) {
        let _ = file.read_exact(&mut buf);

        linedefs.push(LineDef {
            vertex_begin:  i16::from_le_bytes(buf[0..2].try_into().expect("Failed to get bytes from buffer")),
            vertex_end:    i16::from_le_bytes(buf[2..4].try_into().expect("Failed to get bytes from buffer")),
            flags:         i16::from_le_bytes(buf[4..6].try_into().expect("Failed to get bytes from buffer")),
            line_type:     i16::from_le_bytes(buf[6..8].try_into().expect("Failed to get bytes from buffer")),
            sector_tag:    i16::from_le_bytes(buf[8..10].try_into().expect("Failed to get bytes from buffer")),
            sidedef_right: i16::from_le_bytes(buf[10..12].try_into().expect("Failed to get bytes from buffer")),
            sidedef_left:  i16::from_le_bytes(buf[12..14].try_into().expect("Failed to get bytes from buffer")),
        })
    }

    return linedefs;
}

fn decode_maps(file: &mut File, directory: &Vec<DirectoryEntry>) -> Vec<MapData> {
    let map_lumps = vec!(
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

    let mut maps = Vec::new();

    // Collect the raw data
    for d in directory.iter() {
        if d.size == 0 && d.offset > 0 { // this lump is the start of a map
            maps.push(MapData {
                name: String::from(&d.name),
                lumps: Vec::new(),
                linedefs: Vec::new(),
            });
        } else if !maps.is_empty() {
            let mut current_map = maps.last_mut().unwrap();
            match d.name.as_str() {
                "LINEDEFS" => {
                    let linedefs = decode_linedefs(file, d);
                    current_map.linedefs = linedefs;
                }
                _ => ()
            }
        }
    };

    for map in maps.iter() {
        println!("{} has {} linedefs", map.name, map.linedefs.len());
    }

    return maps;
}

impl Wad {
    fn open(filename: &str) -> Wad {
        let mut file = match File::open(filename) {
            Ok(file) => file,
            Err(error) => panic!("Failed to open {}: {}", filename, error),
        };

        let (wad_type, directory_offset, num_directory_entries) = decode_header(&mut file);
        println!("wad_type={:?}, num_directory_entries={}, directory_offset={}", wad_type, num_directory_entries, directory_offset);
        let directory = decode_directory(&mut file, directory_offset, num_directory_entries);

        let maps = decode_maps(&mut file, &directory);

        Wad {
            directory,
            wad_type,
        }
    }
}
