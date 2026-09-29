# Reporting a security problem

If you think you've found a security problem in Cairn, please tell us **privately**, not in a public issue, so it can be fixed before anyone can misuse it.

**How:** on GitHub, open this repository's **Security** tab and choose **Report a vulnerability** ([direct link](https://github.com/thekovie/cairn/security/advisories/new)). Only the maintainers can see what you send.

Please include:

- what the problem is, and what someone could do with it;
- the steps to show it, if you can;
- the Cairn version (**Settings → Updates**, or `cairn.exe --version`).

You don't need a finished proof or a fix. We'll reply as soon as we can, keep you updated, and credit you in the release notes if you'd like.

## Which versions get fixes

Fixes go into the next release. Cairn updates itself (**Settings → Updates**), so the latest release is the one supported.

## What counts

Cairn runs a small web server on each person's own computer, reachable only from that computer, and reads and writes Markdown files with that person's own Windows permissions. Examples of problems we want to hear about:

- another website, program, or person getting into someone's running Cairn;
- a page or picture that runs code, or reaches files outside the documentation folder, when someone opens it;
- Cairn changing or deleting files it shouldn't, or bypassing another person's edit lock in a way that loses work;
- an update being installed that wasn't signed with Cairn's release key.

Not security problems (report these as normal issues): someone with write access to the shared folder changing pages (that's what Windows folder permissions control), or problems that need someone to already control the person's Windows account.
