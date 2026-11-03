mod support;

use crossterm::event::KeyCode;
use support::{Harness, terminal_key};

#[tokio::test]
async fn page_up_and_quit_work_before_response_completion() {
    // Arrange: the source sends text but stays open indefinitely.
    let mut app = Harness::new();
    app.submit("question").await;
    let rows = (1..=30).map(|n| format!("row-{n:02}"))
        .collect::<Vec<_>>().join("\n");
    app.sources[0].send(Ok(rows)).await.unwrap();
    app.wait_screen(|screen| screen.contains("row-30")).await;

    // Act: no completion or additional response chunk is sent.
    app.send(terminal_key(KeyCode::PageUp)).await;
    let reading = app.wait_screen(|screen| screen.contains("row-20")).await;
    app.finish().await.unwrap();

    // Assert: both scroll and quit happened before the source finished.
    assert!(!reading.contains("row-30"), "{reading}");
}

#[tokio::test]
async fn draft_survives_completion_and_becomes_the_next_request() {
    // Arrange: no first chunk arrives, so request startup must not block typing.
    let mut app = Harness::new();
    app.submit("question").await;
    app.wait_screen(|screen| screen.contains("question")).await;

    // Act: draft and try to submit while the first response is still open.
    app.type_text("draft").await;
    app.send(terminal_key(KeyCode::Enter)).await;
    app.sources[0].send(Ok("Hello".into())).await.unwrap();
    app.wait_screen(|screen| screen.contains("Hello") && screen.contains("draft")).await;
    drop(app.sources.remove(0));
    tokio::time::timeout(support::DEADLINE, app.closed.recv()).await.unwrap().unwrap();
    let finished = app.wait_screen(|screen| screen.contains("draft")).await;
    app.send(terminal_key(KeyCode::Enter)).await;
    let next = tokio::time::timeout(support::DEADLINE, app.requests.recv())
        .await.unwrap().unwrap();
    app.finish().await.unwrap();

    // Assert: Enter while busy did not launch a second request or lose the draft.
    assert_eq!(next, "draft");
    assert!(finished.contains("draft"), "{finished}");
}

#[tokio::test]
async fn quit_before_first_chunk_cancels_the_response_source() {
    // Arrange.
    let mut app = Harness::new();
    app.submit("question").await;
    app.wait_screen(|screen| screen.contains("question")).await;

    // Act: the source has not sent even one chunk.
    app.finish().await.unwrap();

    // Assert: the source receiver was dropped, not left running in the background.
    tokio::time::timeout(support::DEADLINE, app.sources[0].closed()).await.unwrap();
}

#[tokio::test]
async fn failure_keeps_partial_text_and_draft_and_allows_another_request() {
    // Arrange.
    let mut app = Harness::new();
    app.submit("question").await;
    app.sources[0].send(Ok("Partial answer".into())).await.unwrap();
    app.wait_screen(|screen| screen.contains("Partial answer")).await;
    app.type_text("retry").await;
    app.wait_screen(|screen| screen.contains("retry")).await;

    // Act: fail only after the draft is visible, not merely queued for input.
    app.sources[0].send(Err(anyhow::anyhow!("upstream failed"))).await.unwrap();
    let failed = app.wait_screen(|screen| {
        screen.contains("upstream failed") && screen.contains("Rupture AI")
    }).await;
    app.send(terminal_key(KeyCode::Enter)).await;
    let retry = tokio::time::timeout(support::DEADLINE, app.requests.recv())
        .await.unwrap().unwrap();
    app.finish().await.unwrap();

    // Assert: failure is visible, old text survives, and the next request is accepted.
    assert!(failed.contains("Partial answer"), "{failed}");
    assert!(failed.contains("retry"), "{failed}");
    assert_eq!(retry, "retry");
}

#[tokio::test]
async fn empty_submission_does_not_start_a_request() {
    // Arrange.
    let mut app = Harness::new();
    app.wait_screen(|screen| screen.contains("Rupture AI")).await;

    // Act: exercise both an empty string and whitespace-only input.
    app.send(terminal_key(KeyCode::Enter)).await;
    app.type_text("   ").await;
    app.send(terminal_key(KeyCode::Enter)).await;
    app.finish().await.unwrap();

    // Assert at the external request boundary after all input events were handled.
    assert!(app.requests.try_recv().is_err());
}

#[tokio::test]
async fn resize_is_processed_during_an_unfinished_response() {
    use rupture_cli::app::Event;
    // Arrange: initial text fits the wide panel but must wrap in the narrow panel.
    let mut app = Harness::new();
    app.submit("question").await;
    app.sources[0].send(Ok(concat!(
        "0123456789ABCDEFGHIJabcdefghijKLMNOPQRST",
        "klmnopqrstUVWXYZ1234uvwxyz5678THE-END!!!",
    ).into())).await.unwrap();
    app.wait_screen(|screen| screen.contains("END!!!")).await;

    // Act: change the terminal's reported size and forward its real resize event.
    app.backend.lock().unwrap().resize(16, 15);
    app.send(Event::Resize).await;
    let narrow = app.wait_screen(|screen| {
        screen.lines().next().unwrap().chars().count() == 16
            && screen.contains("THE-END!!!")
    }).await;
    app.finish().await.unwrap();

    // Assert.
    assert!(!narrow.contains("0123456789"), "{narrow}");
}

#[tokio::test]
async fn wheel_scrolling_holds_then_resumes_following_during_generation() {
    use crossterm::event::{MouseEvent, MouseEventKind, KeyModifiers};
    use rupture_cli::app::Event;
    let wheel = |kind| Event::Mouse(MouseEvent {
        kind, column: 5, row: 5, modifiers: KeyModifiers::NONE,
    });
    // Arrange.
    let mut app = Harness::new();
    app.submit("question").await;
    let rows = (1..=30).map(|n| format!("row-{n:02}"))
        .collect::<Vec<_>>().join("\n");
    app.sources[0].send(Ok(rows)).await.unwrap();
    app.wait_screen(|screen| screen.contains("row-30")).await;

    // Act and assert: wheel up, then consume a chunk without moving the reading view.
    app.send(wheel(MouseEventKind::ScrollUp)).await;
    let reading = app.wait_screen(|screen| screen.contains("row-22")).await;
    app.sources[0].send(Ok("\nLATEST".into())).await.unwrap();
    tokio::time::timeout(support::DEADLINE, app.frames.changed()).await.unwrap().unwrap();
    assert_eq!(support::text(&app.frames.borrow_and_update()), reading);

    // Act and assert: return to the bottom and verify later chunks follow again.
    app.send(wheel(MouseEventKind::ScrollDown)).await;
    app.send(wheel(MouseEventKind::ScrollDown)).await;
    app.wait_screen(|screen| screen.contains("LATEST")).await;
    app.sources[0].send(Ok("\nNEWEST".into())).await.unwrap();
    let latest = app.wait_screen(|screen| screen.contains("NEWEST")).await;
    app.finish().await.unwrap();
    assert!(latest.contains("LATEST"), "{latest}");
}

#[tokio::test]
async fn quit_still_works_when_the_response_producer_is_busy() {
    // Arrange: the producer can supply far more chunks than the bounded queue holds.
    let mut app = Harness::new();
    app.submit("question").await;
    let source = app.sources[0].clone();
    let producer = tokio::spawn(async move {
        for index in 0..100_000 {
            if source.send(Ok("x".into())).await.is_err() { return index; }
        }
        100_000
    });
    app.wait_screen(|screen| screen.contains("xx")).await;

    // Act.
    app.finish().await.unwrap();

    // Assert: queued chunks do not require draining the whole response before quit.
    let produced = tokio::time::timeout(support::DEADLINE, producer).await.unwrap().unwrap();
    assert!(produced < 100_000, "all chunks finished before quit was handled");
}

#[tokio::test]
async fn input_failure_is_reported_and_cancels_the_pending_response() {
    use rupture_cli::app::Event;
    // Arrange.
    let mut app = Harness::new();
    app.submit("question").await;
    app.wait_screen(|screen| screen.contains("question")).await;

    // Act.
    app.send(Event::TerminalInputFailure(std::io::Error::other("input device lost"))).await;
    let error = app.result().await.unwrap_err();

    // Assert: error context survives, and the response source has been dropped.
    assert!(format!("{error:#}").contains("input device lost"));
    tokio::time::timeout(support::DEADLINE, app.sources[0].closed()).await.unwrap();
}

#[tokio::test]
async fn cancelling_the_event_loop_drops_the_response_source() {
    // Arrange.
    let mut app = Harness::new();
    app.submit("question").await;
    app.wait_screen(|screen| screen.contains("question")).await;

    // Act: cancel the loop future itself, not a normal quit event.
    app.abort().await;

    // Assert: cancelling the loop drops its owned response stream.
    tokio::time::timeout(support::DEADLINE, app.sources[0].closed()).await.unwrap();
}

#[tokio::test]
async fn repeated_enter_while_busy_does_not_start_another_reader() {
    // Arrange.
    let mut app = Harness::new();
    app.submit("question").await;
    app.wait_screen(|screen| screen.contains("question")).await;

    // Act: the first source remains open while a new draft is submitted twice.
    app.type_text("draft").await;
    app.send(terminal_key(KeyCode::Enter)).await;
    app.send(terminal_key(KeyCode::Enter)).await;
    app.finish().await.unwrap();

    // Assert: only the initial request reached the external source factory.
    assert!(app.requests.try_recv().is_err());
}

#[tokio::test]
async fn input_channel_closure_stops_the_loop_and_drops_the_response() {
    // Arrange: a request is active, but its external source remains open.
    let mut app = Harness::new();
    app.submit("question").await;
    app.wait_screen(|screen| screen.contains("question")).await;

    // Act: simulate the input producer ending without sending a quit event.
    let (unused_sender, _unused_receiver) = tokio::sync::mpsc::channel(1);
    drop(std::mem::replace(&mut app.tx, unused_sender));
    app.result().await.unwrap();

    // Assert: the source was dropped without needing to finish the response.
    tokio::time::timeout(support::DEADLINE, app.sources[0].closed()).await.unwrap();
}

#[tokio::test]
async fn always_ready_stream_does_not_starve_input_on_the_single_thread_runtime() {
    use futures::{StreamExt, stream};
    // Arrange: unlike the bounded-channel source, this stream never has to wait.
    let stream = stream::iter([Ok("HOT".to_owned())])
        .chain(stream::repeat_with(|| Ok(String::new())));
    let mut app = Harness::with_stream(Box::pin(stream));
    app.submit("question").await;
    app.wait_screen(|screen| screen.contains("HOT")).await;

    // Act: typing and quit must still get runtime execution time.
    app.type_text("draft").await;
    let drafting = app.wait_screen(|screen| screen.contains("draft")).await;
    app.finish().await.unwrap();

    // Assert at the rendered screen and loop-exit boundaries.
    assert!(drafting.contains("HOT"), "{drafting}");
}
