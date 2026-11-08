//! xterm 6 `browser/input/MoveToCell.ts`: the arrow keys an Alt click sends
//! to move the shell cursor. Ported as written, including its quirks, so both
//! clients send the same keys.
use std::iter::repeat_n;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Direction {
    Up,
    Down,
    Right,
    Left,
}

struct Buffer<'a> {
    screen: &'a vt100::Screen,
    rows: i32,
    cols: i32,
    application: bool,
}

impl Buffer<'_> {
    /// `lines.get(row)?.isWrapped`: the row continues the one above it.
    fn is_wrapped(&self, row: i32) -> bool {
        row > 0 && row < self.rows && self.screen.row_wrapped(cell(row - 1))
    }
    fn sequence(&self, direction: Direction, count: i32) -> String {
        let key = match direction {
            Direction::Up => 'A',
            Direction::Down => 'B',
            Direction::Right => 'C',
            Direction::Left => 'D',
        };
        let step = format!("\x1b{}{key}", if self.application { 'O' } else { '[' });
        repeat_n(step, usize::try_from(count).unwrap_or(0)).collect()
    }
    fn wrapped_rows_for_row(&self, mut row: i32) -> i32 {
        let mut count = 0;
        let mut wraps = self.is_wrapped(row);
        while wraps && row >= 0 && row < self.rows {
            count += 1;
            row -= 1;
            wraps = self.is_wrapped(row);
        }
        count
    }
    fn wrapped_rows_count(&self, start_y: i32, target_y: i32) -> i32 {
        let start = start_y - self.wrapped_rows_for_row(start_y);
        let end = target_y - self.wrapped_rows_for_row(target_y);
        let step = if start_y > target_y { -1 } else { 1 };
        (0..(start - end).abs())
            .filter(|i| self.is_wrapped(start + step * i))
            .count() as i32
    }
    fn to_row(&self, start_y: i32, target_y: i32) -> String {
        let start = start_y - self.wrapped_rows_for_row(start_y);
        let end = target_y - self.wrapped_rows_for_row(target_y);
        let direction = if start_y > target_y {
            Direction::Up
        } else {
            Direction::Down
        };
        self.sequence(
            direction,
            (start - end).abs() - self.wrapped_rows_count(start_y, target_y),
        )
    }
    /// `translateBufferLineToString(row, false, start, end).length`.
    fn text_len(&self, row: i32, mut start: i32, end: i32) -> i32 {
        if !(0..self.rows).contains(&row) {
            return 0;
        }
        let mut length = 0;
        while start < end && start < self.cols {
            let (units, width) = match self.screen.cell(cell(row), cell(start)) {
                Some(c) if c.has_contents() => (
                    c.contents().encode_utf16().count() as i32,
                    if c.is_wide() { 2 } else { 1 },
                ),
                _ => (1, 1),
            };
            length += units;
            start += width;
        }
        length
    }
    fn line_len(
        &self,
        mut start_col: i32,
        start_row: i32,
        end_col: i32,
        end_row: i32,
        forward: bool,
    ) -> i32 {
        let (mut col, mut row, mut length) = (start_col, start_row, 0);
        while (col != end_col || row != end_row) && row >= 0 && row < self.rows {
            col += if forward { 1 } else { -1 };
            if forward && col > self.cols - 1 {
                length += self.text_len(row, start_col, col);
                col = 0;
                start_col = 0;
                row += 1;
            } else if !forward && col < 0 {
                length += self.text_len(row, 0, start_col + 1);
                col = self.cols - 1;
                start_col = col;
                row -= 1;
            }
        }
        length + self.text_len(row, start_col, col)
    }
    fn reset_starting_row(&self, start_x: i32, start_y: i32, target_y: i32) -> String {
        if self.to_row(start_y, target_y).is_empty() {
            return String::new();
        }
        let wrapped = start_y - self.wrapped_rows_for_row(start_y);
        self.sequence(
            Direction::Left,
            self.line_len(start_x, start_y, start_x, wrapped, false),
        )
    }
    fn horizontal(&self, start: (i32, i32), target: (i32, i32)) -> Direction {
        let (start_x, start_y) = start;
        let (target_x, target_y) = target;
        // xterm passes targetX as the start row here.
        let row = if self.to_row(target_x, target_y).is_empty() {
            start_y
        } else {
            target_y - self.wrapped_rows_for_row(target_y)
        };
        if (start_x < target_x && row <= target_y) || (start_x >= target_x && row < target_y) {
            Direction::Right
        } else {
            Direction::Left
        }
    }
    fn to_col(&self, start: (i32, i32), target: (i32, i32)) -> String {
        let row = if self.to_row(start.1, target.1).is_empty() {
            start.1
        } else {
            target.1 - self.wrapped_rows_for_row(target.1)
        };
        let direction = self.horizontal(start, target);
        let forward = direction == Direction::Right;
        self.sequence(
            direction,
            self.line_len(start.0, row, target.0, target.1, forward),
        )
    }
}

fn cell(value: i32) -> u16 {
    u16::try_from(value).unwrap_or(0)
}

/// Keys from the cursor to `target`, a viewport `(row, col)`.
pub(super) fn sequence(screen: &vt100::Screen, target: (u16, u16)) -> String {
    let (rows, cols) = screen.size();
    let buffer = Buffer {
        screen,
        rows: i32::from(rows.get()),
        cols: i32::from(cols.get()),
        application: screen.application_cursor(),
    };
    let (start_y, start_x) = screen.cursor_position();
    let (start_x, start_y) = (i32::from(start_x), i32::from(start_y));
    let (target_x, target_y) = (i32::from(target.1), i32::from(target.0));
    // `buffer.hasScrollback`: the normal buffer keeps 1000 rows of history,
    // the alternate buffer none.
    if screen.alternate_screen() {
        return buffer.reset_starting_row(start_x, start_y, target_y)
            + &buffer.to_row(start_y, target_y)
            + &buffer.to_col((start_x, start_y), (target_x, target_y));
    }
    if start_y == target_y {
        let direction = if start_x > target_x {
            Direction::Left
        } else {
            Direction::Right
        };
        return buffer.sequence(direction, (start_x - target_x).abs());
    }
    let direction = if start_y > target_y {
        Direction::Left
    } else {
        Direction::Right
    };
    // colsFromRowEnd + whole rows + 1 + colsFromRowBeginning (currX - 1).
    let (upper_x, lower_x) = if start_y > target_y {
        (target_x, start_x)
    } else {
        (start_x, target_x)
    };
    let cells = (buffer.cols - upper_x)
        + ((start_y - target_y).abs() - 1) * buffer.cols
        + 1
        + (lower_x - 1);
    buffer.sequence(direction, cells)
}
