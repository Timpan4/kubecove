//! Failure modes that need parser state, history eviction or split wire data.
use super::reflow::{History, Parser, resize};
use std::num::NonZeroU16;

fn n(value: u16) -> NonZeroU16 {
    NonZeroU16::new(value).unwrap()
}
fn parser(rows: u16, cols: u16) -> Parser {
    Parser::new_with_callbacks(n(rows), n(cols), 1000, History::default())
}
fn contents(parser: &Parser) -> String {
    let mut screen = parser.screen().clone();
    let history = screen.scrollback_len();
    let mut text = String::new();
    for index in 0..history + usize::from(screen.size().0.get()) {
        let row = if index < history {
            screen.set_scrollback(history - index);
            0
        } else {
            screen.set_scrollback(0);
            u16::try_from(index - history).unwrap()
        };
        text.push_str(
            screen
                .rows(0, screen.size().1.get())
                .nth(usize::from(row))
                .unwrap()
                .as_str(),
        );
        if !screen.row_wrapped(row)
            && !parser
                .wraps
                .contains_key(&(parser.callbacks().discarded + index))
        {
            text.push('\n');
        }
    }
    text.trim_end_matches('\n').to_owned()
}
#[test]
fn resize_keeps_completed_wrapped_lines_and_hard_newlines() {
    let mut terminal = parser(4, 12);
    terminal.process("alpha 界e\u{301}-world\r\nhard line\r\n".as_bytes());
    let before = contents(&terminal);
    resize(&mut terminal, n(5), n(7));
    assert_eq!(contents(&terminal), before);
    resize(&mut terminal, n(4), n(20));
    assert_eq!(contents(&terminal), before);
}
#[test]
fn reflow_keeps_history_and_the_logical_cursor() {
    let mut terminal = parser(4, 12);
    for index in 0..50 {
        terminal.process(format!("line {index} abcdefghijkl\r\n").as_bytes());
    }
    let before = contents(&terminal);
    resize(&mut terminal, n(6), n(8));
    assert_eq!(contents(&terminal), before);
    terminal.process(b"cursor");
    assert!(contents(&terminal).ends_with("cursor"));
}
#[test]
fn reflow_preserves_attributes_and_an_incomplete_wire_character() {
    let mut terminal = parser(4, 12);
    terminal.process(b"\x1b[31mred\r\n\x1b[?2004h\xe7");
    resize(&mut terminal, n(4), n(8));
    terminal.process(b"\x95\x8c");
    assert!(contents(&terminal).starts_with("red\n界"));
    assert!(terminal.screen().bracketed_paste());
    assert_eq!(
        terminal.screen().cell(0, 0).unwrap().fgcolor(),
        vt100::Color::Idx(1)
    );
}
#[test]
fn alternate_screen_does_not_enter_normal_scrollback() {
    let mut terminal = parser(4, 12);
    terminal.process(b"normal\x1b[?1049halt");
    resize(&mut terminal, n(5), n(8));
    terminal.process(b"\x1b[?1049l");
    assert!(terminal.screen().contents().starts_with("normal"));
    assert_eq!(terminal.screen().scrollback_len(), 0);
}

fn tab() -> super::state::ExecTab {
    super::state::ExecTab {
        key: 0,
        container: "app".into(),
        command: vec!["/bin/sh".into()],
        phase: super::state::ExecPhase::Ended(super::state::ExecEnd::Stopped),
        stream: None,
        error: None,
        parser: parser(3, 8),
        selection: None,
        lines: vec![],
        rendered: vec![],
        accessible: "".into(),
        cursor_visible: true,
    }
}
#[test]
fn eviction_clears_selection_instead_of_copying_replacement_rows() {
    let mut tab = tab();
    tab.parser.process(b"selected\r\n");
    tab.selection = Some(super::selection::Selection {
        end: (0, 8),
        ..super::selection::Selection::at((0, 0), false)
    });
    for _ in 0..tab.parser.screen().scrollback_capacity()
        + usize::from(tab.parser.screen().size().0.get())
    {
        tab.parser.process(b"new\r\n");
    }
    tab.refresh_view();
    assert!(tab.selection.is_none());
    assert!(tab.selected_text().is_none());
}
#[test]
fn copy_keeps_explicit_spaces_and_complete_unicode_cells() {
    let mut tab = tab();
    tab.parser.process("界e\u{301}   x\r\nhard\r\n".as_bytes());
    tab.selection = Some(super::selection::Selection {
        end: (1, 4),
        ..super::selection::Selection::at((0, 0), false)
    });
    assert_eq!(
        tab.selected_text().unwrap(),
        if cfg!(target_os = "windows") {
            "界e\u{301}   x\r\nhard"
        } else {
            "界e\u{301}   x\nhard"
        }
    );
}

#[test]
fn erasing_a_wide_wrap_cannot_join_new_hard_lines() {
    let mut tab = tab();
    tab.parser.process("1234567界\r\n".as_bytes());
    tab.parser.process(b"\x1b[2");
    tab.parser.process(b"J\x1b[Hfirst\r\nsecond\r\n");
    tab.selection = Some(super::selection::Selection {
        end: (1, 6),
        ..super::selection::Selection::at((0, 0), false)
    });
    assert_eq!(
        tab.selected_text().unwrap(),
        if cfg!(target_os = "windows") {
            "first\r\nsecond"
        } else {
            "first\nsecond"
        }
    );
}

// Failure mode: the gap a wide character leaves before its wrap is past the wrap
// end, so the forward scan never stops and overflows its column counter.
#[test]
fn double_click_in_a_wide_character_wrap_gap_stays_in_bounds() {
    let mut tab = tab();
    tab.parser.process("abcdefg界\r\n".as_bytes());
    assert_eq!(tab.word((0, 7)).end, (1, 0));
}

// Failure mode: clipboard text with `ESC [201~` ends the bracketed paste, and the
// shell runs the command after it on the pasted newline.
#[test]
fn bracketed_paste_cannot_be_ended_by_pasted_text() {
    let payload = super::selection::paste_payload("echo safe\x1b[201~rm -rf ~\n", true);
    assert!(payload.starts_with("\x1b[200~"));
    assert_eq!(payload.matches('\x1b').count(), 2);
    assert!(payload.ends_with("rm -rf ~\r\x1b[201~"));
}

#[test]
fn double_click_joins_a_word_across_a_wide_character_wrap() {
    let mut tab = tab();
    tab.parser.process("abcdefg界e\u{301}-x\r\n".as_bytes());
    tab.selection = Some(tab.word((1, 1)));
    assert_eq!(tab.selected_text().unwrap(), "abcdefg界e\u{301}-x");
}

// Failure mode: resize rebuilds the screen, so the DECSC cursor and its
// attributes reset and ESC 8 after a width change lands on the wrong cell.
#[test]
fn saved_cursor_and_attributes_survive_a_width_change() {
    let mut terminal = parser(4, 12);
    terminal.process(b"alpha beta gamma\r\n\x1b[31m$ ls\x1b7\x1b[0m\r\nnext");
    resize(&mut terminal, n(4), n(30));
    terminal.process(b"\x1b8X");
    assert_eq!(
        terminal.screen().contents(),
        "alpha beta gamma\n$ lsX\nnext"
    );
    assert_eq!(
        terminal.screen().cell(1, 4).unwrap().fgcolor(),
        vt100::Color::Idx(1)
    );
}

// Failure mode: the saved cell starts a continuation row, and the replay is
// still at the end of the previous row with autowrap pending.
#[test]
fn saved_cursor_at_a_wrap_boundary_restores_on_the_continuation_row() {
    let mut terminal = parser(4, 6);
    terminal.process(b"abcdefg\x08\x1b7\r\nok");
    resize(&mut terminal, n(4), n(3));
    terminal.process(b"\x1b8X");
    assert_eq!(terminal.screen().contents(), "abcdefX\nok");
}

// Failure mode: a wide character wraps with one column still free, so the
// saved cell stays in the gap it leaves on the previous row.
#[test]
fn saved_cursor_follows_a_wide_character_that_wraps_early() {
    let mut terminal = parser(4, 8);
    terminal.process("ab\x1b7界c\r\nok".as_bytes());
    resize(&mut terminal, n(4), n(3));
    terminal.process(b"\x1b8X");
    let screen = terminal.screen();
    assert_eq!(screen.cell(1, 0).unwrap().contents(), "X");
    assert_eq!(screen.cell(0, 2).unwrap().contents(), "");
}

// Failure mode: a wide character's second column has no contents, so a saved
// cursor on it, at the end of a row, maps one column to the right.
#[test]
fn saved_cursor_on_the_second_column_of_a_trailing_wide_character() {
    let mut terminal = parser(4, 6);
    terminal.process("abcde漢\x1b[2;2H\x1b7\x1b[3;1Hok".as_bytes());
    resize(&mut terminal, n(4), n(3));
    terminal.process(b"\x1b8X");
    assert_eq!(terminal.screen().cursor_position(), (2, 2));
}

// Failure mode: SU and CUU honor a DECSTBM region and DECOM, so rows above the
// region survive Clear and the app's margins or origin mode are lost.
#[test]
fn clear_ignores_and_keeps_scroll_margins_and_origin_mode() {
    for origin in [false, true] {
        let mut terminal = parser(5, 8);
        terminal.process(b"a\r\nb\r\nc\r\n\x1b[2;4r");
        if origin {
            terminal.process(b"\x1b[?6h");
        }
        terminal.process(b"\x1b[3;1H$ ");
        terminal.clear();
        assert_eq!(terminal.screen().contents(), "$ ", "origin {origin}");
        terminal.process(b"\x1b[H");
        let home = terminal.screen().cursor_position();
        assert_eq!(home, (u16::from(origin), 0), "origin {origin}: DECOM kept");
        terminal.process(b"\x1b[?6h\x1b[H");
        let top = terminal.screen().cursor_position().0;
        terminal.process(b"\x1b[999H");
        let bottom = terminal.screen().cursor_position().0;
        assert_eq!((top, bottom), (1, 3), "origin {origin}: margins kept");
    }
}

// Failure mode: the margins path places the cursor with CUP, which clamps the
// column, so the next character overwrites the last cell instead of wrapping.
#[test]
fn clear_with_scroll_margins_keeps_a_pending_wrap() {
    let mut terminal = parser(5, 8);
    terminal.process(b"a\r\n\x1b[2;4r\x1b[3;1Habcdefgh");
    terminal.clear();
    terminal.process(b"X");
    assert_eq!(terminal.screen().contents(), "abcdefghX");
}

// Failure mode: the cursor line drops a wide cell that does not fit, but the
// saved-cursor mapping still counts it and moves to the next line.
#[test]
fn saved_cursor_ignores_a_wide_cell_the_cursor_line_drops() {
    let mut terminal = parser(4, 8);
    terminal.process("abc\x1b7界\x1b[2;1Hzz\x1b[1;6H".as_bytes());
    resize(&mut terminal, n(4), n(4));
    terminal.process(b"\x1b8X");
    assert_eq!(terminal.screen().contents(), "abcX\nzz");
}

// Failure mode: the final cursor placement clamps the column, so a pending
// autowrap on a wrapped row is lost, or under DECOM the redraw lands on the
// region's top row and leaves a stray copy of the last cell there.
#[test]
fn clear_places_a_pending_wrap_without_stray_cells() {
    let mut terminal = parser(5, 8);
    terminal.process(b"a\r\nabcdefghij\x1b[A\x1b[8GH");
    terminal.clear();
    terminal.process(b"X");
    assert_eq!(terminal.screen().contents(), "abcdefgHX");

    let mut terminal = parser(5, 8);
    terminal.process(b"a\r\n\x1b[2;4r\x1b[?6h\x1b[2;1Habcdefgh");
    terminal.clear();
    terminal.process(b"X");
    // The documented DECOM exception: the cursor ends on the region's top row.
    assert_eq!(terminal.screen().contents(), "abcdefgh\n       X");
}

fn newline() -> &'static str {
    if cfg!(target_os = "windows") {
        "\r\n"
    } else {
        "\n"
    }
}
// xterm `Terminal.clear()`: history goes and the cursor's row becomes row 0.
#[test]
fn clear_drops_history_and_keeps_the_prompt_row() {
    let mut tab = tab();
    for index in 0..5 {
        tab.parser.process(format!("out {index}\r\n").as_bytes());
    }
    tab.parser.process(b"$ ");
    tab.select_all();
    tab.clear();
    assert!(tab.selection.is_none());
    assert_eq!(tab.parser.screen().cursor_position(), (0, 2));
    tab.parser.process(b"ls\r\nfile\r\n$ ");
    tab.select_all();
    let text = tab.selected_text().unwrap();
    assert!(
        text.starts_with(&format!("$ ls{0}file{0}$", newline())),
        "{text:?}"
    );
    assert!(!text.contains("out"));
}
// Failure mode: the kept row still wraps into the blank row below, so the next
// hard line is copied as part of it.
#[test]
fn clear_on_a_wrapped_row_does_not_join_the_next_hard_line() {
    let mut tab = tab();
    tab.parser.process(b"out\r\n$ abcdefghij\x1b[A");
    // Scrolled into history, viewport row 1 is not the cursor's row.
    tab.parser.process(b"\r\n\r\n\x1b[2A");
    tab.parser.screen_mut().set_scrollback(1);
    tab.clear();
    tab.parser.process(b"\r\nnext");
    tab.select_all();
    let text = tab.selected_text().unwrap();
    assert!(
        text.starts_with(&format!("$ abcdef{}next", newline())),
        "{text:?}"
    );
}
