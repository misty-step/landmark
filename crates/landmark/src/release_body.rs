use crate::*;

pub(crate) fn update_release(args: UpdateReleaseArgs) -> Result<()> {
    let notes = read_nonempty(&args.notes_file)?;
    let provider = GitHubProvider::required(&args.api_base_url, &args.github_token);
    provider.update_release_body(&args.repository, &args.tag, &notes)?;
    Ok(())
}

/// Sentinels bounding the synthesized "What's New" block. Synthesized notes
/// routinely carry their own `## ` subheadings (`## Bug Fixes`, `## Features`),
/// so the section can't be re-found on a later compose by scanning for the
/// next top-level heading — that heuristic mistakes the notes' own subheading
/// for the section boundary and leaves the rest of a prior run's content
/// behind. Marking the block explicitly makes replacement exact regardless of
/// what the notes contain, so repeated synthesis runs against the same
/// release (e.g. a full-mode run and a synthesis-only run both firing off one
/// `release: published` event) converge on the latest notes instead of
/// stacking every run's output.
const WHATS_NEW_START: &str = "<!-- landmark:whats-new:start -->";
const WHATS_NEW_END: &str = "<!-- landmark:whats-new:end -->";

pub(crate) fn compose_release_body(notes: &str, existing: &str) -> String {
    let stripped = strip_existing_whats_new(existing);
    let block = format!(
        "{WHATS_NEW_START}\n## What's New\n\n{}\n{WHATS_NEW_END}",
        notes.trim()
    );
    if stripped.trim().is_empty() {
        format!("{block}\n")
    } else {
        format!("{block}\n\n{}", stripped.trim())
    }
}

pub(crate) fn strip_existing_whats_new(body: &str) -> String {
    if let (Some(start), Some(end)) = (body.find(WHATS_NEW_START), body.find(WHATS_NEW_END))
        && end >= start
    {
        let before = &body[..start];
        let after = &body[end + WHATS_NEW_END.len()..];
        return format!("{before}\n{after}").trim().to_string();
    }
    // Legacy fallback for bodies composed before the sentinel markers existed:
    // best-effort strip by heading boundary. Self-heals to the marker-bounded
    // form on the next synthesis run.
    let mut output = Vec::new();
    let mut skipping = false;
    let mut skipped = false;
    for line in body.lines() {
        if !skipped && line.trim() == "## What's New" {
            skipping = true;
            skipped = true;
            continue;
        }
        if skipping && line.starts_with("## ") {
            skipping = false;
        }
        if !skipping {
            output.push(line);
        }
    }
    output.join("\n").trim().to_string()
}
