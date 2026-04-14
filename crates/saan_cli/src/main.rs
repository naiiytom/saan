/// Top-level CLI commands supported by the Saan toolbelt.
#[derive(Debug, PartialEq)]
pub enum Command {
    /// `saan init` – initialise a new metadata store.
    Init,
    /// `saan prepare` – extract raw metadata from data sources.
    Prepare,
    /// `saan interlace` – define how metadata assets connect.
    Interlace,
    /// `saan apply` – persist the graph to the local store.
    Apply,
    /// `saan inspect` – validate the graph structure.
    Inspect,
    /// `saan view` – launch the WASM visualizer.
    View,
}

/// Parse the first positional argument into a `Command`.
/// Returns `None` when the argument is unrecognised or absent.
pub fn parse_command(args: &[String]) -> Option<Command> {
    match args.first().map(String::as_str) {
        Some("init") => Some(Command::Init),
        Some("prepare") => Some(Command::Prepare),
        Some("interlace") => Some(Command::Interlace),
        Some("apply") => Some(Command::Apply),
        Some("inspect") => Some(Command::Inspect),
        Some("view") => Some(Command::View),
        _ => None,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse_command(&args) {
        Some(cmd) => println!("Running: {:?}", cmd),
        None => {
            eprintln!("Usage: saan <command>");
            eprintln!("Commands: init, prepare, interlace, apply, inspect, view");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &[&str]) -> Vec<String> {
        s.iter().map(|&a| a.to_string()).collect()
    }

    #[test]
    fn parse_init() {
        assert_eq!(parse_command(&args(&["init"])), Some(Command::Init));
    }

    #[test]
    fn parse_prepare() {
        assert_eq!(parse_command(&args(&["prepare"])), Some(Command::Prepare));
    }

    #[test]
    fn parse_interlace() {
        assert_eq!(parse_command(&args(&["interlace"])), Some(Command::Interlace));
    }

    #[test]
    fn parse_apply() {
        assert_eq!(parse_command(&args(&["apply"])), Some(Command::Apply));
    }

    #[test]
    fn parse_inspect() {
        assert_eq!(parse_command(&args(&["inspect"])), Some(Command::Inspect));
    }

    #[test]
    fn parse_view() {
        assert_eq!(parse_command(&args(&["view"])), Some(Command::View));
    }

    #[test]
    fn unknown_command_returns_none() {
        assert_eq!(parse_command(&args(&["foobar"])), None);
    }

    #[test]
    fn empty_args_returns_none() {
        assert_eq!(parse_command(&[]), None);
    }
}
