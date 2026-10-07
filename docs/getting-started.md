# Getting started

This guide takes you from nothing to a first published page. It takes about ten minutes.

## 1. Get Cairn

Download Cairn from <https://github.com/thekovie/cairn/releases/latest>. There's a download for Windows, for a Mac, and for Linux. Cairn keeps itself up to date: see [Updates](configuration.md#updates).

### Windows

- Download `cairn-…-setup.exe` and open it. It installs Cairn **just for you**, so no administrator password is needed. It adds **Cairn** to the Start menu (and, if you tick the box, to the desktop). You can remove it later from **Settings → Apps**; that never touches your documentation.
- Windows may say **Windows protected your PC**, because the program isn't signed with a paid certificate. Choose **More info**, then **Run anyway**.
- Or download `cairn-…-windows-x64.zip` instead, right-click it, choose **Extract All…**, and keep the folder somewhere convenient, such as `C:\Tools\Cairn`. Tip: right-click `cairn.exe` and choose **Send to → Desktop (create shortcut)**.

### Mac

1. Download `cairn-…-macos-universal.zip` and open it. **Cairn** appears next to it (it works on both Apple silicon and Intel Macs).
2. Drag **Cairn** into your **Applications** folder. (Cairn can only update itself from there.)
3. Open **Cairn** from Applications. The first time, the Mac says it can't check Cairn for malicious software, because Cairn isn't signed with a paid Apple certificate. Choose **Done**, open **System Settings → Privacy & Security**, scroll down, choose **Open Anyway** next to the message about Cairn, and confirm. You only do this once.

### Linux

- Download `cairn-…-linux-x86_64.AppImage` and double-click it. If nothing happens, right-click it, choose **Properties → Permissions**, tick **Allow executing file as program** (or run `chmod +x` on it in a terminal), and try again. Keep it somewhere of your own, such as your home folder, so it can update itself.
- Or download `cairn-…-linux-x86_64.tar.gz`, unpack it, and run `./cairn` in a terminal.

To build Cairn yourself from the source code, follow [Building Cairn](building.md).

## 2. Start Cairn

Start **Cairn** from the Start menu (Windows), your Applications folder (Mac), or by double-clicking the AppImage (Linux).

- A small window opens: the black **Cairn** window on Windows, a **Terminal** window on a Mac or Linux. It says **Cairn is running** and shows an address.
- Your web browser opens a new tab with Cairn in it. If it doesn't, copy the address from the small window into your browser.
- **Keep the small window open** while you use Cairn. Closing it stops Cairn (and unlocks any page you're editing). You can also stop Cairn from **Settings → Quit Cairn**.

Cairn only works on your own computer. The address starts with `127.0.0.1`, which means “this computer”. Nobody else on the network can open it.

## 3. Open or create a documentation folder

The first time, Cairn asks what you want to do.

### If your team already has documentation

1. Choose **Open a documentation folder we already use**.
2. Choose **Choose folder…** and pick the shared folder. Picking any folder *inside* the documentation also works: Cairn looks upward to find where the documentation starts.
   - The folder window can appear behind your browser. If you don't see it, check the taskbar at the bottom of the screen (on a Mac, the Dock).
   - Instead, you can type or paste the folder's location and choose **Use this location**. For example `\\server\shared\Documentation` on Windows, `/Volumes/Shared/Documentation` on a Mac, or `/mnt/shared/Documentation` on Linux.
3. Cairn shows the documentation's name and location. Check them, then choose **Open this documentation**.

### To start new documentation

1. Choose **Create a new documentation folder**.
2. Choose where it should live. For a team, pick a shared network folder that everyone can reach.
3. Give the documentation a name, for example “Team Documentation”.
4. Leave **Create a new folder inside the one I chose** selected. This keeps anything already in that folder safe.
5. Choose **Create documentation folder**.

Cairn creates this layout:

```
Documentation/
  shared-docs.json      ← marks this folder as Cairn documentation (don't delete it)
  README.md
  Getting-Started/
  Guides/
  Troubleshooting/
  Reference/
  _system/              ← Cairn's edit locks and earlier versions
```

The folder names are only a starting point. You can add your own folders in Cairn (**New folder**), and rename, move, or delete them with **Organize this folder** at the bottom of each folder (pages have **More actions for this page** under their title). Doing it in Cairn keeps the links between pages working; renaming in File Explorer (or Finder) doesn't.

Next time you start Cairn, it opens the same documentation automatically.

### Setting it up from the command line (optional)

An administrator can create documentation without the browser:

```
cairn init "\\server\shared\Documentation" --name "Team Documentation"
```

(On a Mac or Linux, give the folder's path the same way, for example `cairn init /Volumes/Shared/Documentation --name "Team Documentation"`.)

## 4. Write your first page

1. Choose **New page** (on the left).
2. Type a title, for example “How to connect to the office printer”.
3. Choose a folder, for example **Guides**.
4. Choose what to start from. **Step-by-step guide** gives you headings to fill in.
5. Choose **Create page and start writing**.

The editor opens on the page as it will look.

- Click in the page and type, as in a word processor.
- Use the buttons above to format text: select some words and choose **Bold**, or choose **Heading** or **Bullet list** in the list of text styles.
- To add a screenshot, choose **Insert picture…**, or paste a picture straight into the page.
- Your work is saved on your computer every few seconds. The message at the top says **Draft saved at …**.

When you're happy, choose **Publish changes**, check what Cairn says you changed, and choose **Publish**. Everyone with access to the folder can now read the page.

## 5. Next steps

- [Writing pages](authoring.md) explains formatting, pictures, page details, and earlier versions.
- [Configuration](configuration.md) explains the settings, such as your name, bigger text, and colors.
- If something goes wrong, see [Troubleshooting](troubleshooting.md).
