# Landmark behavior contract

Landmark helps release owners turn a versioned change into understandable,
traceable release notes. The existing [README](README.md) documents its full
command and integration surface; this contract records the notification story
under repair rather than claiming complete specification coverage.

## Release notifications

As a release owner, I can send generated notes to my configured webhook and
distinguish receiver acceptance from rejection, so my automation does not
report a failed notification as delivered. A successful response is HTTP 2xx;
other final responses are failures with an HTTP status, not receiver content.

As a release owner, a temporarily unavailable receiver can recover within a
bounded delivery attempt. Calls have a five-second connection timeout and a
30-second per-attempt deadline, with at most three attempts for transport
failures, HTTP 408/425/429, or HTTP 5xx. Other rejections are not retried.
Optional HMAC-SHA256 signing covers the exact delivered body; an unsigned
notification has no signature header. Best-effort Action notification failures
remain visible without blocking an otherwise successful release.

Owned intent: [webhook delivery repair #285](https://github.com/misty-step/landmark/issues/285).
Verification: the [dogfood skill](skills/landmark-dogfood/SKILL.md), using the
native `replay-action --scenario webhook_delivery_outcomes` loopback journey.
