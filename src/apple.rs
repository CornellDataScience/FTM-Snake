use rand::Rng;
use std::collections::HashSet;

/// Chooses a random unoccupied cell for a new apple.
///
/// Coordinates are zero-indexed `(x, y)` tuples, where `x` is the column and
/// `y` is the row, matching `Point = (i16, i16)` in `main.rs`.
///
/// `occupied` is every cell the apple must not land on: both snakes' bodies
/// (head included) and any apples already on the board. Chain them together
/// at the call site, e.g.
///
/// ```ignore
/// let apple = spawn_apple(
///     SQUARES,
///     SQUARES,
///     snake1.body.iter().chain(snake2.body.iter()).chain(apples.iter()).copied(),
///     &mut rng,
/// );
/// ```
///
/// Returns `None` when every board cell is occupied or the board has no cells.
pub fn spawn_apple(
    width: i16,
    height: i16,
    occupied: impl IntoIterator<Item = (i16, i16)>,
    rng: &mut impl Rng,
) -> Option<(i16, i16)> {
    let occupied: HashSet<(i16, i16)> = occupied.into_iter().collect();

    let mut free_cells = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let cell = (x, y);
            if !occupied.contains(&cell) {
                free_cells.push(cell);
            }
        }
    }

    if free_cells.is_empty() {
        None
    } else {
        Some(free_cells[rng.gen_range(0..free_cells.len())])
    }
}

#[cfg(test)]
mod tests {
    use super::spawn_apple;
    use rand::rngs::StdRng;
    use rand::SeedableRng;
    use std::collections::VecDeque;

    #[test]
    fn apple_is_never_on_a_snake_cell() {
        let snake_cells = [(0, 0), (1, 0), (2, 0), (2, 1)];
        let mut rng = StdRng::seed_from_u64(7);

        for _ in 0..1_000 {
            let apple = spawn_apple(8, 6, snake_cells.iter().copied(), &mut rng).unwrap();
            assert!(!snake_cells.contains(&apple));
        }
    }

    #[test]
    fn apple_is_always_within_bounds() {
        let mut rng = StdRng::seed_from_u64(11);

        for _ in 0..1_000 {
            let (x, y) = spawn_apple(8, 6, std::iter::empty(), &mut rng).unwrap();
            assert!((0..8).contains(&x));
            assert!((0..6).contains(&y));
        }
    }

    #[test]
    fn returns_the_only_free_cell() {
        let mut occupied = Vec::new();
        for y in 0..2 {
            for x in 0..3 {
                if (x, y) != (2, 1) {
                    occupied.push((x, y));
                }
            }
        }
        let mut rng = StdRng::seed_from_u64(13);

        assert_eq!(spawn_apple(3, 2, occupied, &mut rng), Some((2, 1)));
    }

    #[test]
    fn full_board_returns_none() {
        let occupied = [(0, 0), (1, 0), (0, 1), (1, 1)];
        let mut rng = StdRng::seed_from_u64(17);

        assert_eq!(spawn_apple(2, 2, occupied.iter().copied(), &mut rng), None);
    }

    #[test]
    fn apple_is_never_on_an_existing_apple() {
        // 2x1 board: (0, 0) is snake, (1, 0) is an apple, so nothing is free.
        let snake_cells = [(0, 0)];
        let apples = [(1, 0)];
        let mut rng = StdRng::seed_from_u64(23);

        let occupied = snake_cells.iter().chain(apples.iter()).copied();
        assert_eq!(spawn_apple(2, 1, occupied, &mut rng), None);
    }

    #[test]
    fn works_with_two_vecdeque_snakes_like_main() {
        // Mirrors how main.rs stores snake bodies.
        let snake1: VecDeque<(i16, i16)> = VecDeque::from(vec![(0, 0), (1, 0)]);
        let snake2: VecDeque<(i16, i16)> = VecDeque::from(vec![(0, 1)]);
        let apples = [(1, 1)];
        let mut rng = StdRng::seed_from_u64(29);

        // 3x2 board with 4 cells taken leaves only (2, 0) and (2, 1).
        for _ in 0..100 {
            let apple = spawn_apple(
                3,
                2,
                snake1.iter().chain(snake2.iter()).chain(apples.iter()).copied(),
                &mut rng,
            )
            .unwrap();
            assert!(apple == (2, 0) || apple == (2, 1));
        }
    }

    #[test]
    fn same_seed_and_inputs_give_the_same_result() {
        let occupied = [(0, 0), (1, 0), (1, 1), (3, 3)];
        let mut first_rng = StdRng::seed_from_u64(19);
        let mut second_rng = StdRng::seed_from_u64(19);

        let first = spawn_apple(5, 4, occupied.iter().copied(), &mut first_rng);
        let second = spawn_apple(5, 4, occupied.iter().copied(), &mut second_rng);

        assert_eq!(first, second);
    }
}