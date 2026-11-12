use crate::{APPLE_COUNT, Board};

/// Each starting snake gets a square with room for its body and first move.
pub(crate) const SPAWN_SPACING: i16 = 5;

/// Rules chosen once at server startup and reused for every round.
#[derive(Debug, Clone, Copy)]
pub struct GameConfig {
    pub board: Board,
    pub apple_count: usize,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            board: Board::default(),
            apple_count: APPLE_COUNT,
        }
    }
}

impl GameConfig {
    /// Capacity comes from available starting areas, not a fixed player limit.
    pub fn player_capacity(&self) -> usize {
        (self.board.width.max(0) / SPAWN_SPACING) as usize
            * (self.board.height.max(0) / SPAWN_SPACING) as usize
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.board.width < SPAWN_SPACING || self.board.height < SPAWN_SPACING {
            return Err("Board width and height must each be at least 5".into());
        }
        if self.player_capacity() < 2 {
            return Err("The board needs at least two 5 x 5 starting areas".into());
        }
        let cells = self.board.width as usize * self.board.height as usize;
        if self.apple_count > cells {
            return Err("Apple count cannot exceed the number of board cells".into());
        }
        Ok(())
    }

    /// Small explicit parser; add another match arm to expose a new setting.
    pub fn from_args(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut config = Self::default();
        let mut args = args.into_iter();
        while let Some(flag) = args.next() {
            if !matches!(flag.as_str(), "--width" | "--height" | "--apples") {
                return Err(format!("Unknown option: {flag}"));
            }
            let value = args
                .next()
                .ok_or_else(|| format!("Missing value for {flag}"))?;
            match flag.as_str() {
                "--width" => {
                    config.board.width = value
                        .parse()
                        .map_err(|_| "Width must be an integer from 5 to 32767")?;
                }
                "--height" => {
                    config.board.height = value
                        .parse()
                        .map_err(|_| "Height must be an integer from 5 to 32767")?;
                }
                "--apples" => {
                    config.apple_count = value
                        .parse()
                        .map_err(|_| "Apples must be a nonnegative integer")?;
                }
                _ => unreachable!(),
            }
        }
        config.validate()?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<GameConfig, String> {
        GameConfig::from_args(args.iter().map(|arg| arg.to_string()))
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
