# Privacy Policy: Echo VR Launcher Feed bot

*Last updated: September 29, 2026*

This policy explains what personal data the Echo VR Launcher Feed bot ("the Bot") processes,
why, and what your rights are.

## Who is responsible

Mia Hentschel
Email: echo@mia-hentschel.de

## What the Bot does

The Bot copies two kinds of content from the Echo VR community Discord server into public
files that the Echo VR Launcher shows:

1. **The server status post:** one message, posted and kept up to date by a status bot in
   a designated channel.
2. **Community news:** the announcement messages that the server's moderators pick with the
   `/launcher news set` command (at most two at a time).

## What the Bot does not do

- It does **not read the server's conversations.** It is not subscribed to any message
  events. It only fetches the specific messages listed above, by their message ID. It never
  receives, reads or stores any other message.
- It does not post messages, react, send direct messages or change anything in the server.
- It does not collect data for advertising, profiling or tracking, and it sells or shares no
  data. It sets no cookies.

## Data processed, purposes and legal basis

### Content of the selected messages

For the status post and the chosen announcements, the Bot reads the message text, embeds and
the first image.

- **Mentions:** mentions of users, roles and channels in the text are replaced by their
  display names.
- **Not copied:** the message author's name and user IDs.
- **Where it goes:** the result is written as `status.json`, `news.json` and image files to
  `https://files.echovr.de/launcher/feed/`, where it is **publicly accessible**. The Echo VR
  Launcher downloads it from there to show server status and news.

Purpose: showing the server status and community news in the launcher. Legal basis: our
legitimate interest in informing the community (Art. 6(1)(f) GDPR). The announcements are
already published to the community by the server's moderators.

### Use of the Bot's commands

When someone uses a `/launcher` command, Discord sends the Bot what the command needs: the
user's ID and username, the server, the channel and the command's options. The Bot uses this
to carry out the command and to allow it only in the designated channel.

- **What is logged:** the time, username, command and channel, for commands that change
  which messages are shown and for attempts refused outside the designated channel.

Purpose: operating the Bot and tracing changes. Legal basis: legitimate interest in secure
and traceable operation (Art. 6(1)(f) GDPR).

### Stored configuration

The IDs of the channels and messages the Bot shows are stored on the server. They contain no
personal data.

### Web server logs

When the launcher (or anyone) downloads the feed files, the web server at `files.echovr.de`
logs the IP address, time, requested file and the client's user agent. This is done for
operation and security, based on legitimate interest (Art. 6(1)(f) GDPR).

## Retention

| Data | Kept for |
|---|---|
| Feed files (`status.json`, `news.json`, images) | Replaced on every update. Content removed in Discord or unselected by the moderators leaves the feed within about 5 minutes. |
| Bot logs (command uses as above) | 30 days |
| Web server logs | 14 days |
| Configuration (channel and message IDs) | Until changed |

## Recipients and hosting

- **Hosting:** the Bot and the feed files run on a server hosted by Hetzner Online GmbH in
  Germany, who processes the data on our behalf.
- **Public feed:** the feed files are public, so anyone who downloads them receives their
  content.
- **Discord:** Discord processes your data on its platform under its own privacy policy:
  <https://discord.com/privacy>.

## Your rights

Under the GDPR you have the right to:

- **access** your data (Art. 15);
- **rectification** (Art. 16);
- **erasure** (Art. 17);
- **restriction** of processing (Art. 18);
- **data portability** (Art. 20);
- **object** to processing based on legitimate interest (Art. 21).

You can also lodge a complaint with a data protection supervisory authority (Art. 77).

**Removing content:** to have something removed from the launcher feed, ask a server
moderator to unselect the message (`/launcher news clear`), or email
echo@mia-hentschel.de.

## Changes

We update this policy when the Bot changes. The date at the top shows the current version.
