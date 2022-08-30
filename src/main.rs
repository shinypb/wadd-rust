use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    let params = &args[1..];

    println!("Hello, world! Args: {}, {:?}", params.len(), params);

    let v = vec!(1, 2, 3, 4, 5);
    match v.as_slice() {
        []                       => println!("empty"),
        [elem]                   => println!("{}", elem),   // => 1
        [_first, _second, rest @ ..]  => println!("{:?}", rest)  // => &[3, 4, 5]
    }

    match args.as_slice() {
        [_, filename] => handle_command(filename, &String::from("info"), &[]),
        [_, filename, command, rest @ ..] => handle_command(filename, command, rest),
        _ => print_usage_and_exit(&args[0])
    }
}

fn handle_command(filename: &String, command: &String, params: &[String]) {
    println!("Filename: {}, Command: {}, Params: {:?}", filename, command, params)
}

fn print_usage_and_exit(executable: &String) {
    println!("usage: {} /path/to/a/doom.wad command", executable);
}
