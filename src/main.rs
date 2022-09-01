use std::env;
use std::fs::File;
use std::io::{Read};

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

enum LumpType {
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

struct Wad {
    wad_type: WadType
}

fn parse_header(file: &mut File) -> (WadType, i32, i32) {
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

    (wad_type, num_directory_entries, directory_offset)
}

impl Wad {
    fn open(filename: &str) -> Wad {
        let mut f = match File::open(filename) {
            Ok(file) => file,
            Err(error) => panic!("Failed to open {}: {}", filename, error),
        };

        let (wad_type, num_directory_entries, directory_offset) = parse_header(&mut f);
        
        println!("wad_type={:?}, num_directory_entries={}, directory_offset={}", wad_type, num_directory_entries, directory_offset);

        Wad {
            wad_type,
        }
    }
}
