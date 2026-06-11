from __future__ import annotations


def test_publication_policy_allows_valid_required_synthesis(release_policy):
    decision = release_policy.evaluate_publication_policy(
        synthesis_required=True,
        synthesis_strict=False,
        synth_succeeded=True,
        synth_quality="valid",
        synth_failure_stage="",
        synth_failure_message="",
    )

    assert decision.can_update_release is True
    assert decision.succeeded is True
    assert decision.quality == "valid"


def test_publication_policy_blocks_degraded_required_synthesis(release_policy):
    decision = release_policy.evaluate_publication_policy(
        synthesis_required=True,
        synthesis_strict=False,
        synth_succeeded=True,
        synth_quality="degraded",
        synth_failure_stage="",
        synth_failure_message="",
    )

    assert decision.can_update_release is False
    assert decision.succeeded is False
    assert decision.quality == "degraded"
    assert decision.failure_stage == "synthesis_quality"
    assert "degraded" in decision.failure_message


def test_publication_policy_allows_degraded_optional_synthesis(release_policy):
    decision = release_policy.evaluate_publication_policy(
        synthesis_required=False,
        synthesis_strict=False,
        synth_succeeded=True,
        synth_quality="degraded",
        synth_failure_stage="",
        synth_failure_message="",
    )

    assert decision.can_update_release is True
    assert decision.succeeded is True
    assert decision.quality == "degraded"


def test_publication_policy_preserves_synthesis_failure(release_policy):
    decision = release_policy.evaluate_publication_policy(
        synthesis_required=True,
        synthesis_strict=False,
        synth_succeeded=False,
        synth_quality="failed",
        synth_failure_stage="configuration",
        synth_failure_message="missing key",
    )

    assert decision.can_update_release is False
    assert decision.succeeded is False
    assert decision.quality == "failed"
    assert decision.failure_stage == "configuration"
    assert decision.failure_message == "missing key"


def test_summarize_reports_success_only_when_synthesis_and_update_succeed(release_policy):
    result = release_policy.summarize_synthesis_status(
        synthesis_enabled=True,
        released=True,
        synth_succeeded=True,
        synth_quality="valid",
        update_succeeded=True,
        synth_failure_stage="",
        synth_failure_message="",
        update_failure_stage="",
        update_failure_message="",
        artifact_succeeded=True,
        artifact_failure_stage="",
        artifact_failure_message="",
        rss_enabled=False,
        rss_succeeded=False,
        rss_failure_stage="",
        rss_failure_message="",
    )

    assert result.succeeded is True
    assert result.quality == "valid"
    assert result.failure_stage == ""


def test_summarize_preserves_release_update_failure(release_policy):
    result = release_policy.summarize_synthesis_status(
        synthesis_enabled=True,
        released=True,
        synth_succeeded=True,
        synth_quality="valid",
        update_succeeded=False,
        synth_failure_stage="",
        synth_failure_message="",
        update_failure_stage="release_update",
        update_failure_message="patch failed",
        artifact_succeeded=False,
        artifact_failure_stage="",
        artifact_failure_message="",
        rss_enabled=False,
        rss_succeeded=False,
        rss_failure_stage="",
        rss_failure_message="",
    )

    assert result.succeeded is False
    assert result.quality == "failed"
    assert result.failure_stage == "release_update"
    assert result.failure_message == "patch failed"


def test_summarize_preserves_artifact_failure_after_release_update(release_policy):
    result = release_policy.summarize_synthesis_status(
        synthesis_enabled=True,
        released=True,
        synth_succeeded=True,
        synth_quality="valid",
        update_succeeded=True,
        synth_failure_stage="",
        synth_failure_message="",
        update_failure_stage="",
        update_failure_message="",
        artifact_succeeded=False,
        artifact_failure_stage="artifact_write",
        artifact_failure_message="could not write notes",
        rss_enabled=False,
        rss_succeeded=False,
        rss_failure_stage="",
        rss_failure_message="",
    )

    assert result.succeeded is False
    assert result.quality == "failed"
    assert result.failure_stage == "artifact_write"
    assert result.failure_message == "could not write notes"


def test_summarize_preserves_rss_failure_after_artifacts(release_policy):
    result = release_policy.summarize_synthesis_status(
        synthesis_enabled=True,
        released=True,
        synth_succeeded=True,
        synth_quality="valid",
        update_succeeded=True,
        synth_failure_stage="",
        synth_failure_message="",
        update_failure_stage="",
        update_failure_message="",
        artifact_succeeded=True,
        artifact_failure_stage="",
        artifact_failure_message="",
        rss_enabled=True,
        rss_succeeded=False,
        rss_failure_stage="rss_update",
        rss_failure_message="push failed",
    )

    assert result.succeeded is False
    assert result.quality == "failed"
    assert result.failure_stage == "rss_update"
    assert result.failure_message == "push failed"
