//! Unified operation observation: span-based timing, context, and error diagnostics.

use crate::app::observability::context::{EventKind, LogContext};
use crate::app::observability::recorder::MetricsRecorder;
use crate::AppError;
use axum::response::{IntoResponse, Response};
use std::error::Error;
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{error, info_span, Instrument, Span};

/// Wraps a future in a span carrying the operation's classification, context,
/// and timing. On failure, emits a single structured `error!` event with the
/// error type, message, and full source chain.
///
/// The span is entered for the entire duration of `future`, so every child log
/// emitted inside inherits the context fields. `event_duration_ms` and
/// `event_success` are recorded on the span when the future completes.
pub async fn observe<F, R>(
	kind: EventKind,
	name: &str,
	context: &LogContext,
	future: F,
	recorder: &dyn MetricsRecorder,
) -> Result<R, AppError>
where
	F: Future<Output = Result<R, AppError>>,
{
	let span = build_span(kind, name, context);
	let (result, duration_ms) = timed(&span, future).await;

	span.record("event_success", result.is_ok());
	span.record("event_duration_ms", duration_ms);
	if let Err(e) = &result {
		emit_error(&span, e);
	}

	let duration = Duration::from_millis(duration_ms);
	record_metrics(kind, name, recorder, duration, result.as_ref().err());

	result
}

/// Wraps a future in an [`observe`] span and duration metric under the
/// `HttpRequest` event kind.
///
/// Unlike [`observe`], the wrapped future's output isn't a `Result<_, AppError>`
/// — HTTP handlers return their response type directly, and `R: IntoResponse`
/// lets this function convert to a [`Response`] to inspect the status code.
/// Success is derived from `status.is_success()` rather than from a `Result`,
/// since a handler can "complete" while still returning a 4xx/5xx. Domain-level
/// failures inside subsystems continue to flow through their own [`observe`]
/// calls, nested inside this span; this function only captures the
/// request-level view.
///
/// Takes the recorder by [`Arc`] so the returned future owns its reference and
/// is `'static`, satisfying Axum's handler signature.
pub async fn observe_http<F, R>(
	name: &str,
	future: F,
	recorder: Arc<dyn MetricsRecorder>,
) -> Response
where
	F: Future<Output = R>,
	R: IntoResponse,
{
	let span = build_span(EventKind::HttpRequest, name, &LogContext::default());
	let (result, duration_ms) = timed(&span, future).await;
	let response = result.into_response();

	let success = response.status().is_success();
	span.record("event_success", success);
	span.record("event_duration_ms", duration_ms);

	let duration = Duration::from_millis(duration_ms);
	recorder.record_duration(EventKind::HttpRequest, name, duration);
	if !success {
		recorder.record_error(EventKind::HttpRequest, name, response.status().as_str());
	}

	response
}

async fn timed<F: Future>(span: &Span, future: F) -> (F::Output, u64) {
	let start = Instant::now();
	let result = future.instrument(span.clone()).await;
	let ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
	(result, ms)
}

/// Record duration and error metrics through the unified recorder methods,
/// tagged by the operation's event kind.
fn record_metrics(
	kind: EventKind,
	name: &str,
	recorder: &dyn MetricsRecorder,
	duration: std::time::Duration,
	error: Option<&AppError>,
) {
	recorder.record_duration(kind, name, duration);
	if let Some(e) = error {
		recorder.record_error(kind, name, e.error_type());
	}
}

/// Records `context` fields onto a `span`.
fn record_context(span: &Span, context: &LogContext) {
	if let Some(repo) = &context.repository {
		span.record("repository", repo.as_str());
	}
	if let Some(pr) = context.pr_number {
		span.record("pr_number", pr);
	}
	if let Some(user) = &context.github_user {
		span.record("github_user", user.as_str());
	}
	if let Some(id) = context.discord_user_id {
		span.record("discord_user_id", id);
	}
	if let Some(id) = context.channel_id {
		span.record("channel_id", id);
	}
	if let Some(id) = context.guild_id {
		span.record("guild_id", id);
	}
	if let Some(id) = context.thread_id {
		span.record("thread_id", id);
	}
	if let Some(id) = context.installation_id {
		span.record("installation_id", id);
	}
}

/// Record the non-empty fields of `context` onto the currently-active span.
pub fn record_context_on_current_span(context: &LogContext) {
	let span = Span::current();
	record_context(&span, context);
}

/// Build the span carrying the operation's classification and context fields.
fn build_span(kind: EventKind, name: &str, context: &LogContext) -> Span {
	let span = info_span!(
		"observe",
		event_kind = kind.as_str(),
		event_name = name,
		event_success = tracing::field::Empty,
		event_duration_ms = tracing::field::Empty,
		repository = tracing::field::Empty,
		pr_number = tracing::field::Empty,
		github_user = tracing::field::Empty,
		discord_user_id = tracing::field::Empty,
		channel_id = tracing::field::Empty,
		guild_id = tracing::field::Empty,
		thread_id = tracing::field::Empty,
		installation_id = tracing::field::Empty,
	);

	record_context(&span, context);

	span
}

/// Emit a single structured `error!` event with the error type, message, and
/// full source chain, parented to the operation's span.
fn emit_error(span: &Span, err: &AppError) {
	let mut causes = Vec::new();
	let mut cause = err.source();
	while let Some(next) = cause {
		causes.push(next.to_string());
		cause = next.source();
	}

	span.in_scope(|| {
		error!(
			error_type = err.error_type(),
			error_message = %err,
			error_causes = ?causes,
			"operation failed"
		);
	});
}
