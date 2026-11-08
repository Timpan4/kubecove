//! Failure modes that need parser state, history eviction or split wire data.
use super::exec_reflow::{History, Parser, resize};
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

fn tab() -> super::exec_state::ExecTab {
    super::exec_state::ExecTab {
        key: 0,
        container: "app".into(),
        command: vec!["/bin/sh".into()],
        phase: super::exec_state::ExecPhase::Ended(super::exec_state::ExecEnd::Stopped),
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
    tab.selection = Some(super::exec_selection::Selection {
        end: (0, 8),
        ..super::exec_selection::Selection::at((0, 0), false)
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
    tab.selection = Some(super::exec_selection::Selection {
        end: (1, 4),
        ..super::exec_selection::Selection::at((0, 0), false)
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
    tab.selection = Some(super::exec_selection::Selection {
        end: (1, 6),
        ..super::exec_selection::Selection::at((0, 0), false)
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

#[test]
fn double_click_joins_a_word_across_a_wide_character_wrap() {
    let mut tab = tab();
    tab.parser.process("abcdefg界e\u{301}-x\r\n".as_bytes());
    tab.selection = Some(tab.word((1, 1)));
    assert_eq!(tab.selected_text().unwrap(), "abcdefg界e\u{301}-x");
}
