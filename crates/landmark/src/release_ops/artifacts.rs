use crate::*;
pub(crate) fn write_notes_file(content: &str, template: &str, version: &str) -> Result<PathBuf> {
    let path = PathBuf::from(template.replace("{version}", version));
    ensure_parent(&path)?;
    fs::write(&path, content)?;
    Ok(path)
}

pub(crate) fn append_json_entry(
    template: &str,
    artifact: &ReleaseNoteArtifact,
    context: &ReleaseNoteEntryContext,
) -> Result<()> {
    let path = PathBuf::from(template.replace("{version}", &artifact.tag));
    let mut entries = if path.is_file() {
        serde_json::from_str::<Vec<Value>>(&fs::read_to_string(&path)?)?
    } else {
        Vec::new()
    };
    entries.retain(|entry| {
        entry["tag"].as_str() != Some(&artifact.tag)
            && entry["version"].as_str() != Some(&artifact.version)
    });
    entries.push(artifact.json_entry(context));
    ensure_parent(&path)?;
    fs::write(path, serde_json::to_string_pretty(&entries)? + "\n")?;
    Ok(())
}

pub(crate) fn parse_note_sections(markdown: &str) -> Vec<NoteSection> {
    let mut sections = Vec::new();
    let mut current = NoteSection {
        title: "Release notes".to_string(),
        bullets: Vec::new(),
    };
    let link_re = Regex::new(r"\[([^\]]+)\]\(([^)]+)\)").unwrap();
    for line in markdown.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("## ") {
            if !current.bullets.is_empty() || current.title != "Release notes" {
                sections.push(current);
            }
            current = NoteSection {
                title: trimmed.trim_start_matches('#').trim().to_string(),
                bullets: Vec::new(),
            };
            continue;
        }
        if let Some(text) = trimmed.strip_prefix("- ") {
            let links = link_re
                .captures_iter(text)
                .filter_map(|caps| {
                    let href = caps.get(2)?.as_str();
                    Some(NoteLink {
                        label: caps.get(1)?.as_str().to_string(),
                        href: safe_link_href(href)?.to_string(),
                    })
                })
                .collect();
            current.bullets.push(NoteBullet {
                text: markdown_to_plaintext(text),
                links,
            });
        }
    }
    if !current.bullets.is_empty() || current.title != "Release notes" {
        sections.push(current);
    }
    sections
}

pub(crate) fn markdown_to_plaintext(markdown: &str) -> String {
    let mut text = String::new();
    let link_re = Regex::new(r"\[([^\]]+)\]\([^)]+\)").unwrap();
    for line in markdown.lines() {
        let mut line = line
            .trim()
            .trim_start_matches('#')
            .trim()
            .trim_start_matches("- ")
            .to_string();
        line = link_re.replace_all(&line, "$1").to_string();
        line = line.replace("**", "").replace('`', "");
        if !line.is_empty() {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&line);
        }
    }
    text
}

pub(crate) fn markdown_to_html_fragment(markdown: &str) -> String {
    let options = Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES;
    let parser = MarkdownParser::new_ext(markdown, options);
    let mut out = String::new();
    html::push_html(&mut out, parser);
    Regex::new(r#"href="([^"]+)""#)
        .unwrap()
        .replace_all(&out, |caps: &regex::Captures| {
            let href = caps.get(1).unwrap().as_str();
            if safe_link_href(href).is_some() {
                format!("href=\"{href}\"")
            } else {
                "href=\"#\"".to_string()
            }
        })
        .to_string()
}

pub(crate) fn safe_link_href(url: &str) -> Option<&str> {
    let lower = url.trim().to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        Some(url)
    } else {
        None
    }
}

pub(crate) fn update_feed(args: UpdateFeedArgs) -> Result<()> {
    if args.max_entries == 0 {
        return Err("max-entries must be positive".into());
    }
    let notes = read_nonempty(&args.notes_file)?;
    let artifact = ReleaseNoteArtifact::from_markdown(&args.release_tag, &notes);
    let path = args.workspace.join(&args.feed_file);
    let canonical_workspace = args
        .workspace
        .canonicalize()
        .unwrap_or(args.workspace.clone());
    let parent = path.parent().unwrap_or(&args.workspace);
    fs::create_dir_all(parent)?;
    let canonical_parent = parent
        .canonicalize()
        .unwrap_or_else(|_| parent.to_path_buf());
    if !canonical_parent.starts_with(&canonical_workspace) {
        return Err("feed-file must stay inside workspace".into());
    }
    let existing = fs::read_to_string(&path).unwrap_or_default();
    let mut items = parse_existing_feed_items(&existing);
    let new_item = FeedItem {
        title: format!("{} {}", args.repository, args.release_tag),
        link: args.release_url,
        guid: args.release_tag.clone(),
        description: artifact.html,
        pub_date: Utc::now().to_rfc2822(),
    };
    items.retain(|item| item.guid != new_item.guid);
    items.insert(0, new_item);
    items.truncate(args.max_entries);
    let xml = render_feed(
        &args.repository,
        &default_release_url_base(&args.repository),
        &items,
    );
    fs::write(path, xml)?;
    Ok(())
}

#[derive(Clone)]
pub(crate) struct FeedItem {
    pub(crate) title: String,
    pub(crate) link: String,
    pub(crate) guid: String,
    pub(crate) description: String,
    pub(crate) pub_date: String,
}

pub(crate) fn parse_existing_feed_items(xml: &str) -> Vec<FeedItem> {
    let item_re = Regex::new(r"(?s)<item>(.*?)</item>").unwrap();
    item_re
        .captures_iter(xml)
        .map(|cap| {
            let block = cap.get(1).unwrap().as_str();
            FeedItem {
                title: xml_tag(block, "title").unwrap_or_default(),
                link: xml_tag(block, "link").unwrap_or_default(),
                guid: xml_tag(block, "guid").unwrap_or_default(),
                description: xml_tag(block, "description").unwrap_or_default(),
                pub_date: xml_tag(block, "pubDate").unwrap_or_default(),
            }
        })
        .collect()
}

pub(crate) fn xml_tag(block: &str, tag: &str) -> Option<String> {
    let re = Regex::new(&format!(r"(?s)<{tag}>(.*?)</{tag}>")).ok()?;
    Some(re.captures(block)?.get(1)?.as_str().to_string())
}

pub(crate) fn render_feed(repository: &str, channel_link: &str, items: &[FeedItem]) -> String {
    let mut xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<rss version=\"2.0\">\n<channel>\n<title>{}</title>\n<link>{}</link>\n<description>Release notes for {}</description>\n<lastBuildDate>{}</lastBuildDate>\n",
        xml_escape(repository),
        xml_escape(channel_link),
        xml_escape(repository),
        Utc::now().to_rfc2822()
    );
    for item in items {
        xml.push_str(&format!(
            "<item><title>{}</title><link>{}</link><guid>{}</guid><description><![CDATA[{}]]></description><pubDate>{}</pubDate></item>\n",
            xml_escape(&item.title),
            xml_escape(&item.link),
            xml_escape(&item.guid),
            item.description.replace("]]>", "]]]]><![CDATA[>"),
            xml_escape(&item.pub_date)
        ));
    }
    xml.push_str("</channel>\n</rss>\n");
    xml
}

pub(crate) fn default_release_url_base(repository: &str) -> String {
    if repository.contains('/') {
        format!("https://github.com/{repository}")
    } else {
        format!("local://{repository}/releases")
    }
}

pub(crate) fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub(crate) fn notify_webhook(args: NotifyWebhookArgs) -> Result<()> {
    validate_url(&args.webhook_url)?;
    validate_repo(&args.repository)?;
    let notes = read_nonempty(&args.notes_file)?;
    let artifact = ReleaseNoteArtifact::from_markdown(&args.version, &notes);
    let payload = artifact.webhook_payload(&args.repository, &args.release_url);
    let body = payload.to_string();
    let mut command = Command::new("curl");
    command
        .args([
            "-sS",
            "-L",
            "-X",
            "POST",
            "-H",
            "Content-Type: application/json",
        ])
        .arg("-H")
        .arg("User-Agent: landmark")
        .arg("--data-binary")
        .arg("@-");
    if !args.webhook_secret.is_empty() {
        let sig = compute_signature(&args.webhook_secret, body.as_bytes())?;
        command.arg("-H").arg(format!("X-Signature-256: {sig}"));
    }
    command
        .arg(&args.webhook_url)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn()?;
    {
        let mut stdin = child.stdin.take().ok_or("failed to open curl stdin")?;
        stdin.write_all(body.as_bytes())?;
    }
    let output = child.wait_with_output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string().into())
    }
}

pub(crate) fn compute_signature(secret: &str, body: &[u8]) -> Result<String> {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())?;
    mac.update(body);
    Ok(format!(
        "sha256={}",
        hex::encode(mac.finalize().into_bytes())
    ))
}

pub(crate) fn notify_slack(args: NotifySlackArgs) -> Result<()> {
    validate_url(&args.slack_webhook_url)?;
    if !args.slack_webhook_url.contains("hooks.slack.com/") {
        return Err("slack-webhook-url must target hooks.slack.com".into());
    }
    validate_repo(&args.repository)?;
    let notes = read_nonempty(&args.notes_file)?;
    let artifact = ReleaseNoteArtifact::from_markdown(&args.version, &notes);
    let payload = artifact.slack_payload(&args.repository, &args.release_url);
    let response = curl_json("POST", &args.slack_webhook_url, None, Some(&payload))?;
    if (200..300).contains(&response.status) {
        Ok(())
    } else {
        Err(format!("Slack webhook failed with HTTP {}", response.status).into())
    }
}

pub(crate) fn markdown_to_slack(markdown: &str) -> String {
    let text = Regex::new(r"\[([^\]]+)\]\(([^)]+)\)")
        .unwrap()
        .replace_all(markdown, |caps: &regex::Captures| {
            let label = caps.get(1).unwrap().as_str();
            let href = caps.get(2).unwrap().as_str();
            if safe_link_href(href).is_some() {
                format!("<{href}|{label}>")
            } else {
                label.to_string()
            }
        })
        .to_string();
    text.replace("**", "*")
}
