#!/usr/bin/env python3
"""Evaluate Landfall release integrity policy."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path


def parse_bool(value: str | bool | None) -> bool:
    if isinstance(value, bool):
        return value
    return str(value or "").strip().lower() == "true"


def normalize_quality(value: str | None) -> str:
    quality = (value or "").strip().lower()
    return quality if quality in {"valid", "degraded", "failed"} else "failed"


@dataclass(frozen=True)
class PublicationDecision:
    can_update_release: bool
    succeeded: bool
    quality: str
    failure_stage: str
    failure_message: str


@dataclass(frozen=True)
class SynthesisSummary:
    succeeded: bool
    quality: str
    failure_stage: str
    failure_message: str


def evaluate_publication_policy(
    *,
    synthesis_required: bool,
    synthesis_strict: bool,
    synth_succeeded: bool,
    synth_quality: str,
    synth_failure_stage: str,
    synth_failure_message: str,
) -> PublicationDecision:
    quality = normalize_quality(synth_quality)
    if not synth_succeeded:
        return PublicationDecision(
            can_update_release=False,
            succeeded=False,
            quality=quality,
            failure_stage=synth_failure_stage or "synthesis",
            failure_message=synth_failure_message or "Landfall synthesis did not complete successfully.",
        )

    if quality == "degraded" and (synthesis_required or synthesis_strict):
        return PublicationDecision(
            can_update_release=False,
            succeeded=False,
            quality="degraded",
            failure_stage="synthesis_quality",
            failure_message=(
                "Landfall synthesis produced degraded notes; synthesis-required blocks "
                "release-body updates and floating-tag movement."
            ),
        )

    return PublicationDecision(
        can_update_release=True,
        succeeded=True,
        quality=quality,
        failure_stage="",
        failure_message="",
    )


def summarize_synthesis_status(
    *,
    synthesis_enabled: bool,
    released: bool,
    synth_succeeded: bool,
    synth_quality: str,
    update_succeeded: bool,
    synth_failure_stage: str,
    synth_failure_message: str,
    update_failure_stage: str,
    update_failure_message: str,
    artifact_succeeded: bool,
    artifact_failure_stage: str,
    artifact_failure_message: str,
    rss_enabled: bool,
    rss_succeeded: bool,
    rss_failure_stage: str,
    rss_failure_message: str,
) -> SynthesisSummary:
    if not synthesis_enabled or not released:
        return SynthesisSummary(False, "failed", "", "")

    quality = normalize_quality(synth_quality)
    if not synth_succeeded:
        return SynthesisSummary(
            False,
            quality,
            synth_failure_stage or "synthesis",
            synth_failure_message or "Landfall synthesis did not complete successfully.",
        )

    if not update_succeeded:
        return SynthesisSummary(
            False,
            "failed",
            update_failure_stage or "release_update",
            update_failure_message or "Landfall could not update the release body.",
        )

    if not artifact_succeeded:
        return SynthesisSummary(
            False,
            "failed",
            artifact_failure_stage or "artifact_write",
            artifact_failure_message or "Landfall could not write release notes artifacts.",
        )

    if rss_enabled and not rss_succeeded:
        return SynthesisSummary(
            False,
            "failed",
            rss_failure_stage or "rss_update",
            rss_failure_message or "Landfall could not update the RSS release feed.",
        )

    return SynthesisSummary(True, quality, "", "")


def write_outputs(path: str, fields: dict[str, str | bool]) -> None:
    output = Path(path)
    with output.open("a", encoding="utf-8") as handle:
        for key, value in fields.items():
            if isinstance(value, bool):
                rendered = "true" if value else "false"
            else:
                rendered = value
            handle.write(f"{key}={rendered}\n")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Evaluate Landfall release integrity policy.")
    subparsers = parser.add_subparsers(dest="command", required=True)

    publication = subparsers.add_parser("publication")
    publication.add_argument("--synthesis-required", default="false")
    publication.add_argument("--synthesis-strict", default="false")
    publication.add_argument("--synth-succeeded", default="false")
    publication.add_argument("--synth-quality", default="failed")
    publication.add_argument("--synth-failure-stage", default="")
    publication.add_argument("--synth-failure-message", default="")
    publication.add_argument("--github-output", required=True)

    summary = subparsers.add_parser("summary")
    summary.add_argument("--synthesis-enabled", default="false")
    summary.add_argument("--released", default="false")
    summary.add_argument("--synth-succeeded", default="false")
    summary.add_argument("--synth-quality", default="failed")
    summary.add_argument("--update-succeeded", default="false")
    summary.add_argument("--synth-failure-stage", default="")
    summary.add_argument("--synth-failure-message", default="")
    summary.add_argument("--update-failure-stage", default="")
    summary.add_argument("--update-failure-message", default="")
    summary.add_argument("--artifact-succeeded", default="false")
    summary.add_argument("--artifact-failure-stage", default="")
    summary.add_argument("--artifact-failure-message", default="")
    summary.add_argument("--rss-enabled", default="false")
    summary.add_argument("--rss-succeeded", default="false")
    summary.add_argument("--rss-failure-stage", default="")
    summary.add_argument("--rss-failure-message", default="")
    summary.add_argument("--github-output", required=True)

    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.command == "publication":
        decision = evaluate_publication_policy(
            synthesis_required=parse_bool(args.synthesis_required),
            synthesis_strict=parse_bool(args.synthesis_strict),
            synth_succeeded=parse_bool(args.synth_succeeded),
            synth_quality=args.synth_quality,
            synth_failure_stage=args.synth_failure_stage,
            synth_failure_message=args.synth_failure_message,
        )
        write_outputs(
            args.github_output,
            {
                "can_update_release": decision.can_update_release,
                "succeeded": decision.succeeded,
                "quality": decision.quality,
                "failure_stage": decision.failure_stage,
                "failure_message": decision.failure_message,
            },
        )
        return 0

    summary = summarize_synthesis_status(
        synthesis_enabled=parse_bool(args.synthesis_enabled),
        released=parse_bool(args.released),
        synth_succeeded=parse_bool(args.synth_succeeded),
        synth_quality=args.synth_quality,
        update_succeeded=parse_bool(args.update_succeeded),
        synth_failure_stage=args.synth_failure_stage,
        synth_failure_message=args.synth_failure_message,
        update_failure_stage=args.update_failure_stage,
        update_failure_message=args.update_failure_message,
        artifact_succeeded=parse_bool(args.artifact_succeeded),
        artifact_failure_stage=args.artifact_failure_stage,
        artifact_failure_message=args.artifact_failure_message,
        rss_enabled=parse_bool(args.rss_enabled),
        rss_succeeded=parse_bool(args.rss_succeeded),
        rss_failure_stage=args.rss_failure_stage,
        rss_failure_message=args.rss_failure_message,
    )
    write_outputs(
        args.github_output,
        {
            "succeeded": summary.succeeded,
            "quality": summary.quality,
            "failure_stage": summary.failure_stage,
            "failure_message": summary.failure_message,
        },
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
