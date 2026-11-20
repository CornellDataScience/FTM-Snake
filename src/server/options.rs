//! Startup options, separate from the rules reused by every game mode.
use crate::config::{GameConfig, SPAWN_SPACING};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ServerMode {
    /// Original single-server game, with no replication or consensus log.
    #[default]
    None,
    /// Reserved for a future primary/backup implementation.
    PrimaryBackup,
    /// Reserved for the Multi-Paxos teaching implementation.
    Paxos,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ServerOptions {
    pub mode: ServerMode,
    pub game: GameConfig,
}

impl ServerOptions {
    pub const HELP: &str =
        "Usage: server [--NONE | --PB | --PAXOS] [--width 25] [--height 25] [--apples 3]

Modes (choose one):
  --NONE   Original standalone server at 10 moves/second (default).
  --PB     Primary/backup placeholder; exits without starting a server.
  --PAXOS  Multi-Paxos placeholder; exits without starting a server.

Dimensions are in cells. Each player gets a 5 x 5 starting area.
A round starts when at least two players join and everyone is ready.
SNAKE_BIND_ADDRESS overrides the default listener, 0.0.0.0:8080.";

    /// Small explicit parser; add another match arm to expose a new setting.
    pub fn from_args(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut options = Self::default();
        let mut mode_chosen = false;
        let mut args = args.into_iter();
        while let Some(flag) = args.next() {
            let mut value = || {
                args.next()
                    .ok_or_else(|| format!("Missing value for {flag}"))
            };
            let mode = match flag.as_str() {
                "--NONE" => ServerMode::None,
                "--PB" => ServerMode::PrimaryBackup,
                "--PAXOS" => ServerMode::Paxos,
                "--width" => {
                    options.game.board.width = parse_dimension(&value()?, "Width")?;
                    continue;
                }
                "--height" => {
                    options.game.board.height = parse_dimension(&value()?, "Height")?;
                    continue;
                }
                "--apples" => {
                    options.game.apple_count = value()?
                        .parse()
                        .map_err(|_| "Apples must be a nonnegative integer")?;
                    continue;
                }
                _ => return Err(format!("Unknown option: {flag}")),
            };
            if std::mem::replace(&mut mode_chosen, true) {
                return Err("Choose only one server mode: --NONE, --PB or --PAXOS".into());
            }
            options.mode = mode;
        }
        options.game.validate()?;
        Ok(options)
    }
}

fn parse_dimension(value: &str, name: &str) -> Result<i16, String> {
    value.parse().map_err(|_| {
        format!(
            "{name} must be an integer from {SPAWN_SPACING} to {}",
            i16::MAX
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<GameConfig, String> {
        ServerOptions::from_args(args.iter().map(|arg| arg.to_string())).map(|options| options.game)
    }

    #[test]
    fn standalone_is_default_and_modes_can_mix_with_game_options() {
        assert_eq!(ServerOptions::from_args([]).unwrap().mode, ServerMode::None);
        for (flag, mode) in [
            ("--NONE", ServerMode::None),
            ("--PB", ServerMode::PrimaryBackup),
            ("--PAXOS", ServerMode::Paxos),
        ] {
            for args in [
                vec![flag, "--width", "60", "--apples", "0"],
                vec!["--width", "60", flag, "--apples", "0"],
                vec!["--width", "60", "--apples", "0", flag],
            ] {
                let options = ServerOptions::from_args(args.into_iter().map(String::from)).unwrap();
                assert_eq!(options.mode, mode);
                assert_eq!(options.game.board.width, 60);
                assert_eq!(options.game.apple_count, 0);
            }
        }
    }

    #[test]
    fn multiple_mode_flags_are_rejected_instead_of_silently_overriding() {
        for first in ["--NONE", "--PB", "--PAXOS"] {
            for second in ["--NONE", "--PB", "--PAXOS"] {
                let error = ServerOptions::from_args([first.into(), second.into()]).unwrap_err();
                assert!(error.contains("Choose only one server mode"));
            }
        }
    }

    #[test]
    fn defaults_and_custom_options() {
        let defaults = parse(&[]).unwrap();
        assert_eq!(
            (
                defaults.board.width,
                defaults.board.height,
                defaults.apple_count
            ),
            (25, 25, 3)
        );
        let custom = parse(&["--width", "60", "--height", "40", "--apples", "20"]).unwrap();
        assert_eq!(
            (custom.board.width, custom.board.height, custom.apple_count),
            (60, 40, 20)
        );
        assert_eq!(custom.player_capacity(), 96);
        assert_eq!(parse(&["--apples", "0"]).unwrap().apple_count, 0);
    }

    #[test]
    fn rejects_invalid_configuration_before_starting_server() {
        for args in [
            vec!["--unknown", "2"],
            vec!["--width"],
            vec!["--width", "abc"],
            vec!["--width", "0"],
            vec!["--height", "-1"],
            vec!["--width", "32768"],
            vec!["--width", "5", "--height", "5"],
            vec!["--apples", "-1"],
            vec!["--apples", "626"],
        ] {
            assert!(parse(&args).is_err(), "accepted invalid args: {args:?}");
        }
    }
}
