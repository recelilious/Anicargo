# Web Interaction Notes

Updated: 2026-03-22

## Core Principles

- Default visitor flow does not require login.
- Each device gets a stable local device identity.
- Device subscriptions stay on that device unless the user registers and logs in.
- Admin is a separate world under `/admin`.
- Normal user sessions must not open the admin panel.

## Main Navigation

### `/`

- default landing page
- seasonal airing schedule
- top weekday switcher
- auto-focus on today
- vertical anime cards with poster, title, and short tags

### `/search`

- search-first catalog view
- keyword input on top
- follow-up filters for year and tag
- unified result cards matching the schedule style

### `/settings`

- device identity status
- anonymous vs account mode explanation
- user register / login
- basic web preferences

### `/title/:subjectId`

- detail page for one Bangumi subject
- blurred hero background using cover art
- title, tags, summary, infobox details
- subscription action
- episode list

### `/watch/:subjectId/:episodeId`

- episode watch entry
- current phase shows readiness state and future player slot

### `/admin`

- separate admin login only
- no reuse of normal user login
- threshold policy
- replacement window policy
- fansub blacklist / whitelist / priority controls

## Subscription Modes

### Device mode

- default mode
- no account required
- subscriptions bound to `device_id`

### User mode

- enabled after register / login
- subscriptions use the account identity instead of device-only identity
- intended for cross-device continuity later

## First Frontend Goal

The first frontend version should answer these questions clearly for the user:

1. What airs today?
2. Can I find the show I want?
3. What is this show about?
4. Can I subscribe to it right now?
5. If a resource exists, where do I watch it?
6. If I am an admin, where do I control policy and subtitle preference?
