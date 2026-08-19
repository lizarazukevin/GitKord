//! Unified operation observation: span-based timing, context, and error diagnostics.

use crate::app::observability::context::{EventKind, LogContext};
use crate::app::observability::recorder::MetricsRecorder;
use crate::AppError;
use std::error::Error;
use std::future::Future;
use std::time::Instant;
use tracing::{error, info_span, Instrument, Span};

/// Wraps a future in a span carrying the operation's classification, context,
/// and timing. On failure, emits a single structured `error!` event with the
/// error type, message, and full source chain.
///
/// The span is entered for the entire duration of `future`, so every child log
/// emitted inside inherits the context fields. `event.duration_ms` and
/// `event.success` are recorded on the span when the future completes.
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
	let start = Instant::now();
	let result = future.instrument(span.clone()).await;
	let duration = start.elapsed();
	let duration_ms = u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);

	match &result {
		Ok(_) => {
			span.record("event.success", true);
			span.record("event.duration_ms", duration_ms);
		}
		Err(e) => {
			span.record("event.success", false);
			span.record("event.duration_ms", duration_ms);
			emit_error(&span, e);
			recorder.record_error(kind, name, e.error_type());
		}
	}

	recorder.record_duration(kind, name, duration);

	result
}

/// Build the span carrying the operation's classification and context fields.
fn build_span(kind: EventKind, name: &str, context: &LogContext) -> Span {
	let span = info_span!(
		"observe",
		"event.kind" = kind.as_str(),
		"event.name" = name,
		"event.success" = tracing::field::Empty,
		"event.duration_ms" = tracing::field::Empty,
		repository = tracing::field::Empty,
		pr_number = tracing::field::Empty,
		github_user = tracing::field::Empty,
		discord_user_id = tracing::field::Empty,
		channel_id = tracing::field::Empty,
		guild_id = tracing::field::Empty,
		thread_id = tracing::field::Empty,
		installation_id = tracing::field::Empty,
	);

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
			"error.type" = err.error_type(),
			"error.message" = %err,
			"error.causes" = ?causes,
			"operation failed"
		);
	});
}
