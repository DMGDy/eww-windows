use std::{
    io::{Write,BufReader,Read},
    env,
    os::unix::net::UnixStream,
    iter::Peekable,
    str::SplitWhitespace,
};

struct Window {
    title: String,
    info: String,
    address: String,
    tag: u8,
    order: u8 
}

struct Tag {
    tag: u8,
    windows: Vec<Window>,
}

const WORKSPACE_COUNT: usize = 9;

fn peek_until_newline<'a>(iter: &mut Peekable<SplitWhitespace<'a>>) -> String {
    let mut result = Vec::new();
    while let Some(&word) = iter.peek() {
        if word.contains("initialClass:") {
            // If the word contains a newline, split it
            let parts: Vec<&str> = word.split("initialClass:").collect();
            result.push(parts[0]); // Add the part before the newline
            break;
        }
        result.push(word);
        iter.next();
    }
    result.join(" ")
}

fn get_windows() -> Vec<Window> {
    let mut sock = UnixStream::connect(
        format!("{}/hypr/{}/.socket.sock",
            env::var("XDG_RUNTIME_DIR").unwrap(),
            env::var("HYPRLAND_INSTANCE_SIGNATURE").unwrap()
        )).unwrap();

    let _ = sock.write_all(b"clients");

    let mut buff = String::new();
    sock.read_to_string(&mut buff).unwrap();

    let mut iter = buff.split_whitespace().peekable();

    let mut done = false;
    let mut all_windows: Vec<Window> = Vec::new();
    let mut title = String::new();
    let mut info = String::new();
    let mut address = String::new();
    let mut tag: u8 = u8::default();
    let mut order: u8 = u8::default();

    while let Some(key) = iter.next() {
        match key {
            "Window" => {
                address = iter.peek().unwrap().to_string();
            },
            "workspace:" => {
                tag = iter.peek().unwrap().parse().unwrap();
            },
            "title:" => {
                info = peek_until_newline(&mut iter);
                println!("{info}");
            },
            "initialTitle:" => {
                title = iter.peek().unwrap().to_string();
            }
            "focusHistoryID:" => {
                order = iter.peek().unwrap().parse().unwrap();
                done = true;
            }
            _ => {}
        };

        if done == true{
            all_windows.push( Window{
                 title: title.clone(),
                 info: info.clone(),
                 address: address.clone(),
                 tag,
                 order
            });
            done = false;
        };
    }
    all_windows
}

fn main() {
    let all_windows = get_windows();
}
