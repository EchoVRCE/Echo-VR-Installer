# Launcher feed bot

Mirrors the SERVER INFO post and the Community News message(s) from the Echo VR Discord
into `https://files.echovr.de/launcher/feed/` (`status.json`, `news.json` and their
images), which the launcher's Play page reads.

Runs on `files.echo` in `/root/EchoLauncherFeed` as `echo-launcher-feed.service`; logs go
to `/root/log/launcher_feed.log` (one file per day, kept 30 days) and the journal.

It is read-only by design: it receives no message events and only fetches the configured
messages by ID, every 30 s (status) and 5 min (news). [Terms of Service](TERMS.md) ·
[Privacy Policy](PRIVACY.md).

## Discord setup

- Developer Portal → Bot: enable **Message Content Intent** (a switch for bots in fewer than
  100 servers). It only makes the text of fetched messages visible; it grants no rights.
- Invite with the `bot` and `applications.commands` scopes and no permissions. Then allow
  its role only *View Channel* and *Read Message History* in the SERVER INFO channel and in
  each channel news is picked from, and *View Channel* in the control channel. It needs no
  *Send Messages*: command replies are private interaction responses. At startup the bot
  logs a warning for every right beyond these.
- Terms of Service URL / Privacy Policy URL (General Information):
  `https://github.com/EchoVRCE/Echo-VR-Installer/blob/main/server/feed-bot/TERMS.md`
  and `.../PRIVACY.md` (live once this directory is on `main`).

## Commands (Manage Server; ephemeral replies)

They only work in the control channel (`control_channel` in `config.json`, #779349591438524457);
anywhere else the bot answers with a pointer to it. To also hide `/launcher` in other
channels, a server admin can limit it under Server Settings → Integrations → the bot.

- `/launcher news set message:<link or ID> [slot] [channel]`: show that message.
  `main` feeds the banner and the first card, `community` the second card. A bare ID is
  looked up in the announcements channel unless `channel` is given.
- `/launcher news clear slot:<slot>`
- `/launcher news show`
- `/launcher refresh`: export everything now.

The chosen messages are kept in `config.json` next to the bot.

## Deploy

```sh
ssh files.echo 'mkdir -p /root/EchoLauncherFeed'
scp feed_bot.py requirements.txt files.echo:/root/EchoLauncherFeed/
scp echo-launcher-feed.service files.echo:/etc/systemd/system/
ssh files.echo 'cd /root/EchoLauncherFeed && python3 -m venv .venv && .venv/bin/pip install -r requirements.txt'
# The token goes into /root/EchoLauncherFeed/.bot.creds (chmod 600): the bare token,
# TOKEN=... lines or JSON with a "token" key.
ssh files.echo 'cd /root/EchoLauncherFeed && .venv/bin/python feed_bot.py --dump'
ssh files.echo 'systemctl daemon-reload && systemctl enable --now echo-launcher-feed'
```
