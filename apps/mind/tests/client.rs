use std::collections::VecDeque;

use kortex_mind::client::{AskError, Piece, Source, Utf8Stream, collect};
use kortex_runtime::block_on;

/// Scripted response: pieces are handed out one by one, as a network would.
struct Script(VecDeque<Piece>);

impl Script {
    fn new(pieces: Vec<Piece>) -> Self {
        Self(pieces.into())
    }
}

impl Source for Script {
    async fn next(&mut self) -> Piece {
        self.0.pop_front().unwrap_or(Piece::End)
    }
}

fn text(s: &str) -> Piece {
    Piece::Text(s.to_owned())
}

const STREAM: &str = concat!(
    "data: {\"delta\":{\"content\":\"Les\"},\"finish_reason\":null}\n\n",
    "data: {\"delta\":{\"content\":\" chats\"},\"finish_reason\":null}\n\n",
    "data: {\"delta\":{},\"finish_reason\":\"stop\"}\n\n",
    "event: done\ndata: {\"credits\":{\"percent\":91,\"status\":\"ok\",\"resets_in_seconds\":null}}\n\n",
);

#[test]
fn a_streamed_answer_is_assembled_while_events_are_shown_live() {
    // Cut the wire in the middle of an event to mimic network chunks.
    let (head, tail) = STREAM.split_at(40);
    let mut source = Script::new(vec![Piece::Status(200), text(head), text(tail), Piece::End]);
    let mut live = String::new();
    let result = block_on(collect(&mut source, |event| {
        if let kortex_ai::stream::Event::Delta {
            content: Some(c), ..
        } = event
        {
            live.push_str(c);
        }
    }));
    let (turn, credits) = result.unwrap_or_else(|e| unreachable!("{e:?}"));
    assert_eq!(live, "Les chats");
    assert_eq!(turn.content.as_deref(), Some("Les chats"));
    assert_eq!(credits.map(|c| c.percent), Some(91));
}

#[test]
fn streamed_tool_calls_come_out_as_a_complete_turn() {
    let wire = concat!(
        "data: {\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_x\",\"function\":{\"name\":\"text__cleaner\",\"arguments\":\"\"}}]},\"finish_reason\":null}\n\n",
        "data: {\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"text\\\":\\\"@texte1\\\"}\"}}]},\"finish_reason\":null}\n\n",
        "data: {\"delta\":{},\"finish_reason\":\"tool_calls\"}\n\n",
        "event: done\ndata: {\"credits\":{\"percent\":50,\"status\":\"ok\",\"resets_in_seconds\":null}}\n\n",
    );
    let mut source = Script::new(vec![Piece::Status(200), text(wire), Piece::End]);
    let (turn, _) =
        block_on(collect(&mut source, |_| {})).unwrap_or_else(|e| unreachable!("{e:?}"));
    assert_eq!(turn.finish_reason, "tool_calls");
    assert_eq!(turn.tool_calls.len(), 1);
    assert_eq!(turn.tool_calls[0].arguments, "{\"text\":\"@texte1\"}");
}

#[test]
fn gateway_refusals_keep_their_code_and_status() {
    let mut paused = Script::new(vec![
        Piece::Status(429),
        text("{\"error\":\"ai_paused\",\"credits\":{\"percent\":3}}"),
        Piece::End,
    ]);
    assert_eq!(
        block_on(collect(&mut paused, |_| {})).err(),
        Some(AskError {
            status: 429,
            code: "ai_paused".into()
        })
    );
    let mut login = Script::new(vec![
        Piece::Status(401),
        text("{\"error\":\"unauthorized\"}"),
        Piece::End,
    ]);
    assert_eq!(
        block_on(collect(&mut login, |_| {})).err(),
        Some(AskError {
            status: 401,
            code: "unauthorized".into()
        })
    );
    let mut garbage = Script::new(vec![
        Piece::Status(502),
        text("<html>bad gateway</html>"),
        Piece::End,
    ]);
    assert_eq!(
        block_on(collect(&mut garbage, |_| {}))
            .err()
            .map(|e| e.code),
        Some("ai_unavailable".to_owned())
    );
}

#[test]
fn network_failures_truncation_and_stream_errors_are_distinguished() {
    let mut down = Script::new(vec![Piece::Failed]);
    assert_eq!(
        block_on(collect(&mut down, |_| {})).err(),
        Some(AskError {
            status: 0,
            code: "network".into()
        })
    );
    let mut cut = Script::new(vec![
        Piece::Status(200),
        text("data: {\"delta\":{\"content\":\"Bon\"}}\n\n"),
        Piece::Failed,
    ]);
    assert_eq!(
        block_on(collect(&mut cut, |_| {})).err().map(|e| e.code),
        Some("network".to_owned())
    );
    let mut ended = Script::new(vec![
        Piece::Status(200),
        text("data: {\"delta\":{\"content\":\"Bon\"}}\n\n"),
        Piece::End,
    ]);
    assert_eq!(
        block_on(collect(&mut ended, |_| {})).err().map(|e| e.code),
        Some("truncated".to_owned()),
        "no done event: the answer may be incomplete"
    );
    let mut failed = Script::new(vec![
        Piece::Status(200),
        text("event: error\ndata: {\"error\":\"ai_unavailable\"}\n\n"),
    ]);
    assert_eq!(
        block_on(collect(&mut failed, |_| {})).err(),
        Some(AskError {
            status: 200,
            code: "ai_unavailable".into()
        })
    );
}

#[test]
fn utf8_split_across_chunks_is_held_back_not_corrupted() {
    let bytes = "déjà 极 ok".as_bytes();
    let mut decoder = Utf8Stream::default();
    let mut out = String::new();
    for chunk in bytes.chunks(1) {
        out.push_str(&decoder.push(chunk));
    }
    assert_eq!(out, "déjà 极 ok");
    let mut decoder = Utf8Stream::default();
    assert_eq!(
        decoder.push(&[b'a', 0xFF, b'b']),
        "ab",
        "an invalid byte is dropped, the rest survives"
    );
    assert_eq!(decoder.push(&[0xC3]), "");
    assert_eq!(decoder.push(&[0xA9]), "é");
}
