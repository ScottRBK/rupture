use crossterm::event::{MouseEvent, MouseEventKind, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};
use rupture_cli::app::{App, Event};

type TestTerminal = Terminal<TestBackend>;

fn terminal(width: u16, height: u16) -> TestTerminal {
    Terminal::new(TestBackend::new(width, height)).unwrap()
}

fn screen(terminal: &TestTerminal) -> String {
    let buffer = terminal.backend().buffer();
    (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn conversation() -> App {
    let mut app = App::new();
    app.messages.push(
        (1..=20)
            .map(|row| format!("row-{row:02}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    app
}

fn wheel(kind: MouseEventKind) -> Event {
    Event::Mouse(MouseEvent {
        kind,
        column: 5,
        row: 5,
        modifiers: KeyModifiers::NONE,
    })
}

#[tokio::test]
async fn wheel_down_returns_to_newer_text_after_wheel_up() {
    // Arrange: the conversation is taller than the panel.
    let mut terminal = terminal(30, 15);
    let mut app = conversation();
    app.redraw(&mut terminal).unwrap();
    let bottom = screen(&terminal);
    assert!(bottom.contains("row-20"));

    // Act: route real mouse events through the same handler as run().
    app.handle_event(wheel(MouseEventKind::ScrollUp), &mut terminal)
        .await
        .unwrap();
    app.redraw(&mut terminal).unwrap();
    let older = screen(&terminal);
    app.handle_event(wheel(MouseEventKind::ScrollDown), &mut terminal)
        .await
        .unwrap();
    app.redraw(&mut terminal).unwrap();

    // Assert: catch a wheel-down event accidentally routed to scroll_up().
    assert!(older.contains("row-12"), "{older}");
    assert!(!older.contains("row-20"), "{older}");
    assert_eq!(screen(&terminal), bottom);
}

#[test]
fn chunk_without_newline_is_visible_before_response_finishes() {
    // Arrange: create the assistant entry, as the prompt handler does.
    let mut terminal = terminal(30, 15);
    let mut app = App::new();
    app.messages.push(String::new());

    // Act: append a single chunk without a newline or completion event.
    app.append_response_chunk(0, "Hello").unwrap();
    app.redraw(&mut terminal).unwrap();

    // Assert: inspect real rendered output, not the stored message or scroll offset.
    let visible = screen(&terminal);
    assert!(visible.contains("Hello"), "{visible}");
}

fn key(code: crossterm::event::KeyCode) -> Event {
    Event::UserInput(crossterm::event::KeyEvent::new(code, KeyModifiers::NONE))
}

async fn dispatch(app: &mut App, terminal: &mut TestTerminal, event: Event) {
    app.handle_event(event, terminal).await.unwrap();
    app.redraw(terminal).unwrap();
}

#[test]
fn successive_chunks_extend_one_response_and_follow_the_latest_text() {
    // Arrange.
    let mut terminal = terminal(30, 15);
    let mut app = conversation();
    app.redraw(&mut terminal).unwrap();

    // Act: append chunks, including one with no newline.
    app.append_response_chunk(0, "\nHello").unwrap();
    app.redraw(&mut terminal).unwrap();
    app.append_response_chunk(0, " there").unwrap();
    app.redraw(&mut terminal).unwrap();

    // Assert: chunks stay on one line, appear once, and displace old text.
    let visible = screen(&terminal);
    assert_eq!(visible.matches("Hello there").count(), 1, "{visible}");
    assert!(!visible.contains("row-01"), "{visible}");
}

#[tokio::test]
async fn page_up_holds_reading_position_when_a_chunk_arrives() {
    use crossterm::event::KeyCode;
    // Arrange.
    let mut terminal = terminal(30, 15);
    let mut app = conversation();
    app.redraw(&mut terminal).unwrap();
    dispatch(&mut app, &mut terminal, key(KeyCode::PageUp)).await;
    let before = screen(&terminal);
    assert!(before.contains("row-10"), "{before}");

    // Act.
    app.append_response_chunk(0, "\nLATEST").unwrap();
    app.redraw(&mut terminal).unwrap();

    // Assert: append does not pull the reader back to the bottom.
    assert_eq!(screen(&terminal), before);
}

#[tokio::test]
async fn wheel_up_holds_reading_position_when_a_chunk_arrives() {
    // Arrange.
    let mut terminal = terminal(30, 15);
    let mut app = conversation();
    app.redraw(&mut terminal).unwrap();
    dispatch(&mut app, &mut terminal, wheel(MouseEventKind::ScrollUp)).await;
    let before = screen(&terminal);
    assert!(before.contains("row-12"), "{before}");

    // Act.
    app.append_response_chunk(0, "\nLATEST").unwrap();
    app.redraw(&mut terminal).unwrap();

    // Assert.
    assert_eq!(screen(&terminal), before);
}

#[tokio::test]
async fn page_down_to_bottom_resumes_following() {
    use crossterm::event::KeyCode;
    // Arrange.
    let mut terminal = terminal(30, 15);
    let mut app = conversation();
    app.redraw(&mut terminal).unwrap();
    dispatch(&mut app, &mut terminal, key(KeyCode::PageUp)).await;

    // Act.
    dispatch(&mut app, &mut terminal, key(KeyCode::PageDown)).await;
    app.append_response_chunk(0, "\nLATEST").unwrap();
    app.redraw(&mut terminal).unwrap();

    // Assert.
    let visible = screen(&terminal);
    assert!(visible.contains("LATEST"), "{visible}");
}

#[tokio::test]
async fn wheel_down_to_bottom_resumes_following() {
    // Arrange.
    let mut terminal = terminal(30, 15);
    let mut app = conversation();
    app.redraw(&mut terminal).unwrap();
    dispatch(&mut app, &mut terminal, wheel(MouseEventKind::ScrollUp)).await;

    // Act.
    dispatch(&mut app, &mut terminal, wheel(MouseEventKind::ScrollDown)).await;
    app.append_response_chunk(0, "\nLATEST").unwrap();
    app.redraw(&mut terminal).unwrap();

    // Assert.
    let visible = screen(&terminal);
    assert!(visible.contains("LATEST"), "{visible}");
}

#[tokio::test]
async fn end_jumps_to_latest_and_resumes_following() {
    use crossterm::event::KeyCode;
    // Arrange.
    let mut terminal = terminal(30, 15);
    let mut app = conversation();
    app.redraw(&mut terminal).unwrap();
    dispatch(&mut app, &mut terminal, key(KeyCode::PageUp)).await;

    // Act.
    dispatch(&mut app, &mut terminal, key(KeyCode::End)).await;
    let after_end = screen(&terminal);
    app.append_response_chunk(0, "\nLATEST").unwrap();
    app.redraw(&mut terminal).unwrap();

    // Assert: both the immediate jump and subsequent following work.
    assert!(after_end.contains("row-20"), "{after_end}");
    let visible = screen(&terminal);
    assert!(visible.contains("LATEST"), "{visible}");
}

#[tokio::test]
async fn scrolling_beyond_either_end_keeps_content_visible() {
    use crossterm::event::KeyCode;
    // Arrange.
    let mut terminal = terminal(30, 15);
    let mut app = conversation();
    app.redraw(&mut terminal).unwrap();
    let bottom = screen(&terminal);

    // Act and assert: repeatedly scroll beyond the bottom.
    for _ in 0..10 {
        dispatch(&mut app, &mut terminal, key(KeyCode::PageDown)).await;
    }
    assert_eq!(screen(&terminal), bottom);

    // Act and assert: repeatedly scroll beyond the top.
    for _ in 0..10 {
        dispatch(&mut app, &mut terminal, key(KeyCode::PageUp)).await;
    }
    let top = screen(&terminal);
    assert!(top.contains("row-01"), "{top}");
    assert!(top.contains("row-06"), "{top}");
    assert!(!top.contains("row-20"), "{top}");
}

#[tokio::test]
async fn no_op_scroll_on_short_content_does_not_disable_following() {
    use crossterm::event::KeyCode;
    // Arrange.
    let mut terminal = terminal(30, 15);
    let mut app = App::new();
    app.messages.push("Hello".into());
    app.redraw(&mut terminal).unwrap();
    let before = screen(&terminal);

    // Act and assert: neither direction should move a short conversation.
    for event in [
        key(KeyCode::PageUp),
        key(KeyCode::PageDown),
        wheel(MouseEventKind::ScrollUp),
        wheel(MouseEventKind::ScrollDown),
    ] {
        dispatch(&mut app, &mut terminal, event).await;
        assert_eq!(screen(&terminal), before);
    }

    // Act and assert: later output must still follow automatically.
    app.append_response_chunk(0, "\none\ntwo\nthree\nfour\nfive\nsix\nLATEST")
        .unwrap();
    app.redraw(&mut terminal).unwrap();
    let visible = screen(&terminal);
    assert!(visible.contains("LATEST"), "{visible}");
}

fn wrapped_response() -> App {
    let mut app = App::new();
    app.messages.push(concat!(
        "0123456789ABCDEFGHIJabcdefghijKLMNOPQRST",
        "klmnopqrstUVWXYZ1234uvwxyz5678THE-END!!!",
    ).into());
    app
}

#[test]
fn wrapped_height_uses_inner_width_and_does_not_hide_final_row() {
    // Arrange: panel inner width is ten columns, with six visible rows.
    let mut terminal = terminal(16, 15);
    let mut app = wrapped_response();

    // Act: eight wrapped rows, with no explicit newlines.
    app.redraw(&mut terminal).unwrap();

    // Assert: catches measuring at the outer width that includes the borders.
    let visible = screen(&terminal);
    assert!(visible.contains("THE-END!!!"), "{visible}");
    assert!(!visible.contains("0123456789"), "{visible}");
}

#[tokio::test]
async fn resize_to_narrower_terminal_rewraps_and_follows_latest() {
    // Arrange.
    let mut terminal = terminal(40, 15);
    let mut app = wrapped_response();
    app.redraw(&mut terminal).unwrap();

    // Act: the backend reports a new size, just as a real terminal would.
    terminal.backend_mut().resize(16, 15);
    dispatch(&mut app, &mut terminal, Event::Resize).await;

    // Assert.
    let visible = screen(&terminal);
    assert!(visible.contains("THE-END!!!"), "{visible}");
}

#[tokio::test]
async fn growing_terminal_clamps_paused_offset_to_valid_content() {
    use crossterm::event::KeyCode;
    // Arrange.
    let mut terminal = terminal(30, 15);
    let mut app = conversation();
    app.redraw(&mut terminal).unwrap();
    dispatch(&mut app, &mut terminal, key(KeyCode::PageUp)).await;

    // Act: grow enough to show the entire conversation.
    terminal.backend_mut().resize(30, 35);
    dispatch(&mut app, &mut terminal, Event::Resize).await;

    // Assert: no stale offset hides the start or end.
    let visible = screen(&terminal);
    assert!(visible.contains("row-01"), "{visible}");
    assert!(visible.contains("row-20"), "{visible}");
}

#[tokio::test]
async fn tiny_and_empty_panels_recover_after_resize() {
    // Arrange.
    let mut terminal = terminal(0, 0);
    let mut app = App::new();
    app.messages.push("Hello\nworld".into());

    // Act: exercise panels with no usable inner area, then restore the size.
    for (width, height) in [(0, 0), (1, 1), (5, 5), (30, 15)] {
        terminal.backend_mut().resize(width, height);
        dispatch(&mut app, &mut terminal, Event::Resize).await;
    }

    // Assert.
    let visible = screen(&terminal);
    assert!(visible.contains("Hello"), "{visible}");
    assert!(visible.contains("world"), "{visible}");
}

#[test]
fn natural_word_wrapping_keeps_the_latest_word_visible() {
    // Arrange.
    let mut terminal = terminal(16, 15);
    let mut app = App::new();
    app.messages.push(
        "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda omega".into(),
    );

    // Act.
    app.redraw(&mut terminal).unwrap();

    // Assert.
    let visible = screen(&terminal);
    assert!(visible.contains("omega"), "{visible}");
    assert!(!visible.contains("alpha"), "{visible}");
}

#[test]
fn wide_unicode_and_explicit_newlines_keep_the_tail_visible() {
    // Arrange: each five-character CJK group occupies ten terminal columns.
    let mut terminal = terminal(16, 15);
    let mut app = App::new();
    app.messages.push("你好世界啊".repeat(7) + "\nEND");

    // Act.
    app.redraw(&mut terminal).unwrap();

    // Assert: check cells directly; wide characters occupy continuation cells.
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer[(3, 3)].symbol(), "你");
    assert_eq!(buffer[(11, 3)].symbol(), "啊");
    let visible = screen(&terminal);
    assert!(visible.contains("END"), "{visible}");
}

#[tokio::test]
async fn growing_input_area_keeps_latest_conversation_text_visible() {
    use crossterm::event::{KeyCode, KeyEvent};
    // Arrange.
    let mut terminal = terminal(30, 15);
    let mut app = conversation();
    app.redraw(&mut terminal).unwrap();

    // Act: real multiline-input commands reduce the conversation area height.
    for _ in 0..2 {
        let event = Event::UserInput(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL));
        dispatch(&mut app, &mut terminal, event).await;
    }

    // Assert: the remaining conversation rows still show the tail.
    let visible = screen(&terminal);
    assert!(visible.contains("row-20"), "{visible}");
    assert!(!visible.contains("row-15"), "{visible}");
}

#[tokio::test]
async fn key_release_does_not_scroll_the_conversation() {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
    // Arrange.
    let mut terminal = terminal(30, 15);
    let mut app = conversation();
    app.redraw(&mut terminal).unwrap();
    let before = screen(&terminal);
    let event = Event::UserInput(KeyEvent::new_with_kind(
        KeyCode::PageUp,
        KeyModifiers::NONE,
        KeyEventKind::Release,
    ));

    // Act.
    dispatch(&mut app, &mut terminal, event).await;

    // Assert.
    assert_eq!(screen(&terminal), before);
}

#[test]
fn appending_to_a_missing_response_reports_an_error() {
    // Arrange.
    let mut app = App::new();

    // Act.
    let result = app.append_response_chunk(0, "Hello");

    // Assert: an invalid response index must not panic or silently discard text.
    assert!(result.is_err());
}

#[test]
fn chunks_extend_the_assistant_entry_without_changing_the_user_message() {
    // Arrange: the response is not the first conversation entry.
    let mut terminal = terminal(30, 15);
    let mut app = App::new();
    app.messages = vec!["Question".into(), String::new()];

    // Act.
    app.append_response_chunk(1, "Hel").unwrap();
    app.redraw(&mut terminal).unwrap();
    app.append_response_chunk(1, "lo").unwrap();
    app.redraw(&mut terminal).unwrap();

    // Assert: user and assistant stay on separate rows, with no duplicate response.
    let visible = screen(&terminal);
    assert_eq!(visible.matches("Question").count(), 1, "{visible}");
    assert_eq!(visible.matches("Hello").count(), 1, "{visible}");
    assert!(!visible.contains("QuestionHello"), "{visible}");
}
