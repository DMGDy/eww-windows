use std::{
    io::{Write,Read,BufRead,BufReader},
    env,
    os::unix::net::UnixStream,
    iter::Peekable,
    str::SplitWhitespace,
    collections::BTreeMap,
};

const EVENTS: [&str;5] = [
            "workspace",
            "activewindow",
            "openwindow",
            "closewindow",
            "movewindow",
];

/* for this program:
 *  tag: the index of the workspace
 *  workspace: the actual thing containing the windows
 */

/* Window: metadata assoicated to an open window on hyprland
 *
 * name: the desktop name of the program
 * info: the secondary title or "information" of the window
 * pid: pid associated with the window open
 * tag: the workspace index th window exists on
 * order: 0 meaning active, the order of when it was used
 */
struct Window {
    name: String,
    info: String,
    address: String,
    class: String,
    tag: usize,
    order: usize 
}

// sorted by order (revent activity)
type AllWindows = Vec<Window>;

 /* windows: vector of windows in order of activity
  * order: the order of this workspace in activity
 */
struct Workspace {
    windows: Vec<Window>,
    tag: usize,
    order:  usize,
    active: bool 
}

type Workspaces = BTreeMap<usize,Workspace>;

fn peek_until_newline<'a>(iter: &mut Peekable<SplitWhitespace<'a>>,next_line: &str) -> String {
    let mut result = Vec::new();
    while let Some(&word) = iter.peek() {
        if word.contains(next_line) {
            let parts: Vec<&str> = word.split(next_line).collect();
            result.push(parts[0]); 
            break;
        }
        result.push(word);
        iter.next();
    }
    result.join(" ")
}

fn get_windows() -> Vec<Window>{
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
    let mut all_windows: Vec<Window>  = Vec::new();
    let mut name = String::with_capacity(32);
    let mut info = String::with_capacity(64);
    let mut class = String::with_capacity(32);
    let mut address = String::with_capacity(8);
    let mut tag = usize::default();
    let mut order = usize::default();

    while let Some(key) = iter.next() {
        match key {
            "workspace:" => {
                tag = iter.peek().unwrap().parse().unwrap();
            },
            "title:" => {
                info = peek_until_newline(&mut iter,"initialClass:").trim_end().to_string();
                info = match info {
                    init_class if init_class.contains(".pdf") =>
                        init_class.split("/").last().expect("PDF Document").to_string(),
                    _ => info
                }
            },
            "initialTitle:" => {
                name = peek_until_newline(&mut iter,"pid:").trim_end().to_string();
                // special cases for ugly initialTitles
                name = match name {
                    title if title.contains("Chromium") =>  "Chromium".to_string(),
                    title if title.contains("OBS") =>  "OBS Studio".to_string(),
                    title if title.contains(".pdf") => 
                        title.split("/").last().expect("Document").to_string(),
                    title if title.contains("WhatsApp") => "WhatsApp".to_string(),
                    
                    _ => name 
                        
                }
            }
            "focusHistoryID:" => {
                order = iter.peek().unwrap().parse().unwrap();
                done = true;
            }
            "Window" => {
                address = iter.peek().unwrap().to_string();
            },
            "class:" => {
                class = peek_until_newline(&mut iter, "title:").trim_end().to_string();
            }

            _ => {}
        };

        if done == true{
            all_windows.push( Window{
                 name: name.clone(),
                 info: info.clone(),
                 address: address.clone(),
                 class: class.clone(),
                 tag,
                 order
            });
            done = false;
        };
    }
    all_windows.sort_by(|win1,win2| win1.order.cmp(&win2.order));
    all_windows
}

fn assign_tags_to_win(all_wins: AllWindows) -> Workspaces {
    let mut workspaces: Workspaces = BTreeMap::new();

    let mut order:usize = 0;
    for window in all_wins{
        workspaces.entry(window.tag)
            .or_insert( (|| { 
                let w = Workspace {
                    windows: Vec::new(),
                    active: window.order == 0,
                    tag: window.tag,
                    order,
                };
                order += 1;
                w
            })()).windows.push(window);
    }
   workspaces 
}

fn gen_eww_widget(workspace: &Workspaces) {
    let mut sorted_workspaces: Vec<_> = workspace
        .into_iter()
        .collect();
    // sort by recently used
    sorted_workspaces.sort_by(|w1,w2| w1.1.order.cmp(&w2.1.order));

    print!("(box :class \"window-container\" \
                 :space-evenly false");
    for (_,workspace) in sorted_workspaces {
        print!("(box ");

        if workspace.active {
            print!(":class \"active-workspace\" ")
        } else {
            print!(":class \"workspace\" ");
        }
        print!(":space-evenly false ");
        print!("(label :class \"tag-id\" :text \"{}\" )",workspace.tag);
        for window in &workspace.windows {
            print!("(button :class \"window-tab\"");
                print!(":onclick \
                    \"$HOME/.config/eww/eww-windows/target/release/eww-windows {}\" \
                    ",window.address);
                print!("(box ");
                if window.order == 0 {
                    print!(":class \"active-window\" ");
                } else {
                    print!(":class \"inactive-window\" ");
                }
                print!(":tooltip \"{}\" ",window.info);
                print!(":space-evenly false \
                (box :class \"win-icon\" \
                :style \
                \"background-image:\
                url('icons/{}.svg');\") \
                (box :class \"win-title\" \
                (label :limit-width 16 \
                :text \"{}\" ))",window.class,window.name);
                print!("))")
        }
        print!(")")
    }
    println!(")");
}

/* read hyprland socket2 to see if there is 
 * activity on workspace or winndow change
*/ 
fn is_activity()  -> bool {
    let sock = UnixStream::connect(
        format!("{}/hypr/{}/.socket2.sock",
            env::var("XDG_RUNTIME_DIR").unwrap(),
            env::var("HYPRLAND_INSTANCE_SIGNATURE").unwrap()
        )).unwrap();

    let mut buffer = String::new();
    // for some reason reading socket2 must be buffered
    let mut reader = BufReader::new(sock);
    loop {
        match reader.read_line(&mut buffer) {
            Ok(_) =>{
                for line in buffer.lines() {
                    
                    let event = line 
                        .split(">>")
                        .next()
                        .unwrap();

                    if EVENTS.contains(&event) {
                        return true
                    }
                }
            },
            Err(_) => {}
        }
    }

}

fn swich_window(adr: String) {
    let mut sock = UnixStream::connect(
        format!("{}/hypr/{}/.socket.sock",
            env::var("XDG_RUNTIME_DIR").unwrap(),
            env::var("HYPRLAND_INSTANCE_SIGNATURE").unwrap()
        )).unwrap();

    
    let _ = sock.write_all(format!(
            "dispatch focuswindow address:0x{adr}"
    ).as_bytes());

}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        swich_window(args[1].clone());
        std::process::exit(0)
    }
    loop {
        if is_activity() {
            let all_windows = get_windows();
            let workspaces = assign_tags_to_win(all_windows);
            gen_eww_widget(&workspaces);
        }
    }

}
