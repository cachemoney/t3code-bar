CREATE TABLE orchestration_v2_projection_threads (
 thread_id TEXT PRIMARY KEY, title TEXT NOT NULL, project_id TEXT NOT NULL,
 created_at TEXT NOT NULL, updated_at TEXT NOT NULL, payload_json TEXT NOT NULL,
 interaction_mode TEXT, deleted_at TEXT, archived_at TEXT);
CREATE TABLE projection_projects (project_id TEXT PRIMARY KEY, title TEXT, project_icon_json TEXT, favicon_path TEXT, workspace_root TEXT);
CREATE TABLE orchestration_v2_projection_runs (
 thread_id TEXT, status TEXT, payload_json TEXT, ordinal INTEGER, run_id TEXT,
 requested_at TEXT, completed_at TEXT);
CREATE TABLE orchestration_v2_projection_runtime_requests (thread_id TEXT, status TEXT, kind TEXT);
CREATE TABLE projection_thread_pull_requests (thread_id TEXT, number INTEGER);
CREATE TABLE orchestration_v2_projection_provider_threads (thread_id TEXT, payload_json TEXT, last_run_ordinal INTEGER);
CREATE TABLE orchestration_v2_projection_messages (thread_id TEXT, role TEXT, created_at TEXT, updated_at TEXT, payload_json TEXT);
CREATE TABLE orchestration_v2_projection_turn_items (
 thread_id TEXT, run_id TEXT, type TEXT, status TEXT, node_id TEXT, payload_json TEXT,
 updated_at TEXT, ordinal INTEGER, turn_item_id TEXT);
INSERT INTO projection_projects VALUES ('fixture-project', 'Fixture project', NULL, NULL, '/fixture/workspace');
INSERT INTO orchestration_v2_projection_threads VALUES
 ('approval', 'Review a proposed action', 'fixture-project', '2026-11-04T00:00:00Z', '2026-11-06T09:00:00Z', '{"pinnedAt":"2026-11-04T00:00:00Z","pinOrderKey":"a","branch":"feature/fixture"}', 'default', NULL, NULL),
 ('input', 'Answer a question', 'fixture-project', '2026-11-04T00:00:00Z', '2026-11-06T09:00:00Z', '{"activeOrderKey":"a"}', 'default', NULL, NULL),
 ('request', 'Guest message', 'fixture-project', '2026-11-04T00:00:00Z', '2026-11-06T09:00:00Z', '{"activeOrderKey":"b","guestShare":{"messages":[{"status":"pending"}]}}', 'default', NULL, NULL),
 ('working', 'Working thread', 'fixture-project', '2026-11-04T00:00:00Z', '2026-11-06T09:00:00Z', '{"activeOrderKey":"c"}', 'default', NULL, NULL),
 ('limited', 'Usage limit reached', 'fixture-project', '2026-11-04T00:00:00Z', '2026-11-06T09:00:00Z', '{"activeOrderKey":"d"}', 'default', NULL, NULL),
 ('waiting', 'Waiting on another thread', 'fixture-project', '2026-11-04T00:00:00Z', '2026-11-06T09:00:00Z', '{"activeOrderKey":"e"}', 'default', NULL, NULL),
 ('done', 'Unread completion', 'fixture-project', '2026-11-04T00:00:00Z', '2026-11-06T09:00:00Z', '{"activeOrderKey":"f","lastVisitedAt":"2026-11-04T00:00:00Z"}', 'default', NULL, NULL),
 ('archived', 'Excluded archived thread', 'fixture-project', '2026-11-04T00:00:00Z', '2026-11-06T09:00:00Z', '{}', 'default', NULL, '2026-11-06T09:00:00Z'),
 ('subagent', 'Excluded child thread', 'fixture-project', '2026-11-04T00:00:00Z', '2026-11-06T09:00:00Z', '{"lineage":{"relationshipToParent":"subagent"}}', 'default', NULL, NULL),
 ('snoozed', 'Excluded snoozed thread', 'fixture-project', '2026-11-04T00:00:00Z', '2026-11-06T09:00:00Z', '{"snoozedAt":"2026-11-06T09:00:00Z","snoozedUntil":"2099-11-06T09:00:00Z"}', 'default', NULL, NULL);
INSERT INTO orchestration_v2_projection_runtime_requests VALUES ('approval', 'pending', 'approval'), ('input', 'pending', 'user_input');
INSERT INTO orchestration_v2_projection_runs VALUES
 ('working', 'running', '{"providerInstanceId":"codex","rootNodeId":"root"}', 1, 'run-working', '2026-11-06T09:00:00Z', NULL),
 ('limited', 'failed', '{"providerInstanceId":"codex","rootNodeId":"root"}', 1, 'run-limited', '2026-11-06T09:00:00Z', '2026-11-06T09:01:00Z'),
 ('waiting', 'running', '{"providerInstanceId":"codex","rootNodeId":"root"}', 1, 'run-waiting', '2026-11-06T09:00:00Z', NULL),
 ('done', 'completed', '{"providerInstanceId":"codex","rootNodeId":"root"}', 1, 'run-done', '2026-11-06T09:00:00Z', '2026-11-06T09:01:00Z');
INSERT INTO orchestration_v2_projection_turn_items VALUES
 ('limited', 'run-limited', 'error', 'failed', 'root', '{"failure":{"class":"usage_limit","resetAt":"2026-11-06T11:00:00Z"}}', '2026-11-06T09:01:00Z', 1, 'item-limited'),
 ('waiting', 'run-waiting', 'dynamic_tool', 'waiting', 'root', '{"toolName":"t3_thread_wait"}', '2026-11-06T09:01:00Z', 1, 'item-waiting');
INSERT INTO orchestration_v2_projection_messages VALUES ('approval', 'assistant', '2026-11-06T09:00:00Z', '2026-11-06T09:00:00Z', '{"text":"## Fixture reply\nThe proposed change is ready for review.\n```swift\nsecret code omitted\n```\nNo personal data is present."}');
