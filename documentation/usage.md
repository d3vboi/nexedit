# Nexedit Usage

To launch Nexedit, run the following command:



nexedit [dir | file1 file2 ...]


> **Key Bindings Overview**: The following sections provide an overview of Nexedit's key bindings and functionality. For a comprehensive list of all available commands and their associated key bindings, run `application::display_default_keymap` from the command mode.

## Exiting Nexedit

Before starting to use Nexedit, it's essential to know how to quit the application. In normal mode, press `Q` (Shift+q) to exit Nexedit.

> **Note**: Nexedit will close modified buffers without warning when quitting. Since these are powerful editing tools, you'll often want to close buffers beforehand using `q` (which prompts if the buffer has unsaved changes) until the workspace is empty.

## Working with Files

If you didn't specify any file paths when launching Nexedit, you'll see a splash screen. To find and edit files, enter open mode by pressing `Space`.

> **Warning**: This will recursively index the current directory and all its subdirectories. Use open mode only in project directories, not system-wide paths like `/` or `~`.

### File Finder

Nexedit's file finder works differently than most. Instead of using fuzzy string matching, it relies on string fragments. Rather than typing full words, enter fragments of the file path separated by spaces:



mod app op --> src/__mod__els/__app__lications/__mod__es/__op__en.rs


The search terms must occur in the file path, which generally produces fewer but more accurate results than fuzzy matching. The order of tokens doesn't matter; you can include fragments from parent directory names after the file name fragments.

> **Note**: Pressing backspace will delete the entire last token instead of the last character. Since tokens are typically larger, it's often easier to re-enter the last entry than to correct it.

### Selecting and Opening Files

Once the file you're looking for appears in the results, select it using the up and down arrow keys, then press `Enter`. The file finder has its own insert and normal modes. Hitting `esc` will grey out the input area and expose the following key bindings:

| Key         | Action                  |
|-------------|-------------------------|
| `Space/Enter` | Open the selected file  |
| `j`           | Select the next result  |
| `k`           | Select the previous result |
| `i`           | Edit the search query   |
| `esc`         | Leave open mode         |

> **Tip**: The search/select UI pattern used in open mode is reused elsewhere in Nexedit, with the same fragment matching and insert/normal sub-mode behavior. Get familiar with it; it'll be useful for other features.

### Exclusions

By default, Nexedit's open mode doesn't index `.git` directories. If you want to change this behavior, you can redefine the exclusion patterns in the application preferences.

## Closing Files

In normal mode, press `q` to close the current buffer. If the file has unsaved changes, you'll be prompted to confirm before closing.

## Saving Files

Press `s` to save the current buffer. The UI will indicate when a buffer has unsaved modifications: its path will be rendered in bold with an asterisk, and the normal mode indicator will be orange. These indicators are cleared after saving or if the buffer is reverted to an unmodified state using undo or reload.

## Creating New Files

To create a new file, start by opening a new, empty buffer by pressing `B`. When you're ready, press `s` to save it; Nexedit will realize it has no path and prompt you to enter one, after which the buffer will be written to disk.

## Movement

In normal mode, scroll up and down using the `,` and `m` keys, respectively.

For cursor movement, the standard `h`, `j`, `k`, `l` movement commands are available, along with `w`, `b` for word navigation. For more advanced movement, you'll want to use jump mode.

### Jump Mode

Press `f` to switch to jump mode. On-screen elements will be prefixed with a two-character jump token. Type the characters to jump to the associated element.



jump mode


> **Tip**: Jump mode won't target one-character elements. Use `'` to switch to a single-character version instead. This mode has a more restricted scope, ideal for jumping to smaller, nearby elements.

### Jumping to Symbols

For files with syntax support, you can jump to class, method, and function definitions using symbol mode. In normal mode, hit `Enter` to use the symbol finder, which works identically to open mode.

### Jumping to a Specific Line

You can also move the cursor to a specific line using `g`, which will prompt for a target line number.

## Working with Text

### Inserting Text

Use `i` to enter insert mode. When you're done adding text, hit `esc` to return to normal mode.

### Editing Text

In normal mode, you can interact with text using the following keys:

| Key         | Action                                         |
|-------------|------------------------------------------------|
| `Backspace` | Delete the character to the left of the cursor |
| `x`         | Delete the character to the right of the cursor|
| `d`         | Delete from the cursor to the end of the word  |
| `c`         | Change the text from the cursor to the end of the word |
| `y`         | Copy the current line                          |

### Selecting Text

To start a text selection range, use `v`. Move the cursor using movement keys, then delete, change, or copy the selected text. To select entire lines of text, use `V` instead.

> **Tip**: Configuring your terminal to use a vertical bar cursor instead of a block can make edit operations and text selection more intuitive, though this is a matter of personal preference.

### Using the Clipboard

Nexedit has built-in support for the system clipboard, with no additional configuration or external dependencies required. Use the following keys to interact with the clipboard:

| Key | Action                                                  |
|-----|----------------------------------------------------------|
| `y` | Copy the current selection (if present) or line         |
| `p` | Paste at the cursor                                     |
| `P` | Paste on the line above                                 |

> **Note**: Like in Vim, whenever data is removed or changed in the buffer (e.g., changing a word, deleting the current line), it's copied to the clipboard.

## Running Commands

Under the hood, Nexedit's functionality is exposed through a set of commands, and the UI is driven by a simple key-to-command mapping. You can run any command directly by switching to command mode (`:` from normal mode), which will bring up a search prompt. To browse the full list of available commands, run `application::display_available_commands` to open the complete set in a new buffer.

> **Tip**: Command mode is not primarily for discovery; it's a handy way to trigger infrequently-used functionality that doesn't merit a dedicated key binding (e.g., converting tabs to spaces).

## Search

You can search using `/` to enter a query. If matches are found, the cursor will move to the first match (relative to its current position). Navigate to the next/previous match using `n` and `N`, respectively. Searches will wrap once the end of the file is reached.

Most of the expected keybindings will work: `c` to change the selected content, `d` to delete it, `p` to paste the buffer contents.

## Replace

Nexedit doesn't currently have a proper search and replace workflow; you can't specify a replacement value after searching. However, you can accomplish this with a workaround:

1. Replace the first occurrence manually (`c` to replace the current result)
2. Copy the updated content (`v` to enter select mode, `y` to copy selected content)
3. Start the search again (`n` to find the next result)
4. Paste to replace the content (`p`)

> **Warning**: Nexedit doesn't currently support advanced search options (regular expressions, case sensitivity, recursive file search, etc.). These features will be added in the future.

## Suspend

It can be handy to temporarily leave Nexedit, interact with your shell, and then resume editing. In normal mode, hit `z` to suspend Nexedit and return to your shell, and run `fg` to resume it when you're ready to edit again.

## Git Integration

Nexedit provides basic Git integration. The lower-right portion of the status bar displays the current buffer's status:

- `[untracked]`: the file has never been added to the repository
- `[ok]`: the file is unmodified (matches the repository version)
- `[modified]`: the file has local modifications
- `[staged]`: the file has local modifications, all of which are staged for commit
- `[partially staged]`: the file has local modifications, some of which are staged for commit

### Staging Changes

You can use the `=` key to stage the current file for commit. This feature doesn't currently support staging line ranges.

### Copying GitHub URLs

When collaborating with others, it can be useful to share a link to the file you're working on. The `R` key can be used to copy the current file's GitHub URL. If in select-line mode, the selected line range will also be included in the URL.

> **Note**: This feature assumes that the GitHub remote is configured as `origin`.