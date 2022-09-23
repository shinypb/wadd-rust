mod wadd;

use std::env;
use std::fs::File;
use std::io::Write;
use std::process::exit;

use svg::node::element::path::Data;
use svg::node::element::{Line, Path};
use svg::Document;
use wadd::Wad;

use crate::wadd::{LineDef, Sector, Vertex, WadType};

fn main() {
    let args: Vec<String> = env::args().collect();

    match args.as_slice() {
        [_, filename] => handle_command(filename, &String::from("info"), &[]),
        [_, filename, command, rest @ ..] => handle_command(filename, command, rest),
        _ => print_usage_and_exit(),
    }
}

fn handle_command(filename: &String, command: &str, params: &[String]) {
    let wad = match Wad::open(filename) {
        Ok(wad) => wad,
        Err(err) => {
            println!("Error reading WAD: {}", err);
            std::process::exit(1);
        }
    };

    match command {
        "info" => show_info(&wad),
        "maps" => list_maps(&wad),
        "svg" => match params.first() {
            Some(map_name) => extract_map(&wad, &map_name),
            None => {
                println!("Dumping all maps...");
                for map in &wad.maps {
                    extract_map(&wad, &map.name)
                }
            }
        },
        _ => {
            println!("Sorry, I don't know how to {}.", command);
            std::process::exit(1);
        }
    }
}

fn extract_map(wad: &Wad, map_name: &str) {
    let map = wad
        .maps
        .iter()
        .find(|map| map.name == map_name)
        .expect("That map does not exist.");

    let lines: Vec<(Vertex, Vertex)> = map
        .linedefs
        .iter()
        .map(|linedef| {
            let v1 = map.vertexes[linedef.vertex_begin as usize];
            let v2 = map.vertexes[linedef.vertex_end as usize];

            (v1, v2)
        })
        .collect();

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
    type SectorLine = (Vertex, Vertex, LineDef);
    #[derive(Debug)]
    struct RenderableSector {
        lines: Vec<SectorLine>,
        sector: Sector,
    }
    let sectors: Vec<RenderableSector> = map
        .sectors
        .iter()
        .enumerate()
        .map(|(sector_index, sector)| {
            let sector_lines: Vec<LineDef> = map
                .linedefs
                .iter()
                .filter(|linedef| {
                    if linedef.sidedef_right >= 0 {
                        let sidedef_right = map.sidedefs[linedef.sidedef_right as usize].clone();
                        if (sidedef_right.sector as usize) == sector_index {
                            return true;
                        }
                    }
                    if linedef.sidedef_left >= 0 {
                        let sidedef_left = map.sidedefs[linedef.sidedef_left as usize].clone();
                        if (sidedef_left.sector as usize) == sector_index {
                            return true;
                        }
                    }
                    false
                })
                .map(|linedef| linedef.clone())
                .collect();
            let vertex_lines: Vec<SectorLine> = sector_lines
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
                lines: vertex_lines,
                sector: sector.clone(),
            }
        })
        .collect();

    // Write it out
    println!(
        "{} has {} sectors and is {}x{}",
        &map_name,
        sectors.len(),
        width,
        height
    );
    let mut doc = Document::new().set("viewBox", format!("0 0 {} {}", width, height));

    // Debugging feature: only show a particular subset of sectors; leave empty to include all
    // sectors (as usual).
    let debugging_sector_filter: Vec<usize> = vec![];

    for (sector_id, sector) in sectors.iter().enumerate() {
        if !debugging_sector_filter.is_empty() && !debugging_sector_filter.contains(&sector_id) {
            continue;
        }
        println!("\nSector {} has {} lines:", sector_id, sector.lines.len());
        for (from_v, to_v, _) in &sector.lines {
            println!(
                "({}, {}) -> ({}, {})",
                from_v.x + offset_x,
                from_v.y + offset_y,
                to_v.x + offset_x,
                to_v.y + offset_y
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
            // TODO: There is currently something wrong with this logic, leading to orphan lines not
            // getting added to the path. Sector 0 of E4M9 demonstrates this pretty well.
            if pending_lines.len() < 2 {
                println!(
                    "WARNING: Sector {} only has {} lines left",
                    sector_id,
                    pending_lines.len()
                );
            }

            let (initial_v, mut to_v, _) = pending_lines.remove(0);

            data = data
                .move_to((initial_v.x + offset_x, initial_v.y + offset_y))
                .line_to((to_v.x + offset_x, to_v.y + offset_y));

            while let Some(next_line_idx) =
                pending_lines
                    .iter()
                    .position(|(other_from_v, other_to_v, _)| {
                        // Look for any other lines that share our `to_v` vertex, regardless of direction
                        other_from_v == &to_v || other_to_v == &to_v // handle backwards lines, e.g. E1M1's sector 2
                    })
            {
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

    // Draw sector lines; we do this as a separate pass at the end so they are drawn on top of the
    // filled paths.
    for (sector_id, sector) in sectors.iter().enumerate() {
        if !debugging_sector_filter.is_empty() && !debugging_sector_filter.contains(&sector_id) {
            continue;
        }
        for (from_v, to_v, linedef) in sector.lines.iter() {
            let mut line = Line::new()
                .set("x1", from_v.x + offset_x)
                .set("y1", from_v.y + offset_y)
                .set("x2", to_v.x + offset_x)
                .set("y2", to_v.y + offset_y);
            if linedef.sidedef_left < 0 || linedef.sidedef_right < 0 {
                // one-sided line
                line = line.set("stroke", "red").set("stroke-width", "2");
            } else {
                // two-sided line
                line = line
                    .set("stroke", "rgba(255, 0, 0, 0.25)")
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
    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en-US">
<head>
    <meta charset="utf-8">
    <style>
        html {{
            background: black;
            color: white;
            /* https://stackoverflow.com/posts/35362074/revisions */
            background-image: linear-gradient(45deg, #333 25%, transparent 25%), linear-gradient(-45deg, #333 25%, transparent 25%), linear-gradient(45deg, transparent 75%, #333 75%), linear-gradient(-45deg, transparent 75%, #333 75%);
            background-size: 20px 20px;
            background-position: 0 0, 0 10px, 10px -10px, -10px 0px;
        }}
    </style>
    <meta name="viewport" content="width=device-width, initial-scale=1.0, viewport-fit=cover">
</head>
<body>
    {}
</body>
"#,
        doc.to_string()
    );

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
        println!(
            "- {} ({} linedefs, {} sectors, {} things, {} vertexes)",
            map.name,
            map.linedefs.len(),
            map.sectors.len(),
            map.things.len(),
            map.vertexes.len()
        );
    }
}

fn show_info(wad: &Wad) {
    let wad_type = match wad.wad_type {
        WadType::IWAD => "IWAD",
        WadType::PWAD => "PWAD",
    };
    println!(
        "{} with {} lumps in its directory:",
        &wad_type,
        wad.directory.len()
    );
    for d in &wad.directory {
        if d.size > 0 {
            println!(
                "- {: <8}\t{} bytes starting at {}",
                d.name, d.size, d.offset
            );
        } else {
            println!("- {: <8}\t(empty lump)", d.name);
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
    println!(
        "  (if no map name is specified, every map in the WAD will be extracted automatically)"
    );

    exit(255);
}
