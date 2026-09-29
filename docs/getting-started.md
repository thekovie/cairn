# Getting started

This guide takes you from nothing to a first published page. It takes about ten minutes.

## 1. Get Cairn

Cairn is a single program, `cairn.exe`. The easiest way to get it is the installer:

- Download the latest `cairn-…-setup.exe` from <https://github.com/thekovie/cairn/releases/latest> and open it. It installs Cairn **just for you**, so no administrator password is needed. It adds **Cairn** to the Start menu (and, if you tick the box, to the desktop). You can remove it later from **Settings → Apps**; that never touches your documentation.
- Windows may say **Windows protected your PC**, because the program isn't signed with a paid certificate. Choose **More info**, then **Run anyway**.

Other ways:

- Download `cairn-…-windows-x64.zip` instead, right-click it, choose **Extract All…**, and keep the folder somewhere convenient, such as `C:\Tools\Cairn`. Tip: right-click `cairn.exe` and choose **Send to → Desktop (create shortcut)**.
- If someone gave you `cairn.exe`, copy it somewhere convenient.
- To build it yourself from the source code, follow [Building cairn.exe](building.md). The result is `target\release\cairn.exe`.

Cairn keeps itself up to date: see [Updates](configuration.md#updates).

## 2. Start Cairn

Choose **Cairn** in the Start menu (or double-click `cairn.exe`).

- A small black window opens. It says **Cairn is running** and shows an address.
- Your web browser opens a new tab with Cairn in it. If it doesn't, copy the address from the small window into your browser.
- **Keep the small window open** while you use Cairn. Closing it stops Cairn. You can also stop Cairn from **Settings → Quit Cairn**.

Cairn only works on your own computer. The address starts with `127.0.0.1`, which means “this computer”. Nobody else on the network can open it.

## 3. Open or create a documentation folder

The first time, Cairn asks what you want to do.

### If your team already has documentation

1. Choose **Open a documentation folder we already use**.
2. Choose **Choose folder…** and pick the shared folder. Picking any folder *inside* the documentation also works: Cairn looks upward to find where the documentation starts.
   - The folder window can appear behind your browser. If you don't see it, check the taskbar at the bottom of the screen.
   - Instead, you can type or paste the folder's location, such as `\\server\shared\Documentation`, and choose **Use this location**.
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

The folder names are only a starting point. You can add your own folders in Cairn (**New folder**), and rename, move, or delete them with **Organize this folder** at the bottom of each folder (pages have **More actions for this page** under their title). Doing it in Cairn keeps the links between pages working; renaming in File Explorer doesn't.

Next time you start Cairn, it opens the same documentation automatically.

### Setting it up from the command line (optional)

An administrator can create documentation without the browser:

```
cairn init "\\server\shared\Documentation" --name "Team Documentation"
```

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
