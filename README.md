# Dotty

Dotty is a command-line dotfile manager. It currently moves files and directories into a dotfiles repository, manages them using symbolic links, and checks whether those links and their sources are in the expected state.

## Commands

| Command | Description |
| --- | --- |
| `dotty init <repo-dir>` | Initialize Dotty with a new or existing dotfiles repository. |
| `dotty manage <target>` | Add a file or directory to the repository and manage it via a symlink. |
| `dotty manage <target> --as <source>` | Specify a custom repository-relative source path. |
| `dotty forget <target>` | Stop managing a target. |
| `dotty ls` | List managed sources, targets, and their status. |
| `dotty test` | Check all managed entries and report issues. |
| `dotty test <target>` | Check one managed entry by its target path. |
| `dotty completions <shell>` | Generate shell completions. |

`dotty test` returns exit code `0` when healthy, `1` when unhealthy, and `2` on errors.

## Storage layout

For a repository initialized at `~/dotfiles`, the layout might look like:

```text
~/dotfiles/
├── .dotty/
│   └── manifest.toml      # Portable list of managed entries
├── zshrc                  # Source for ~/.zshrc
└── ghostty/
    └── config             # Source for ~/.config/ghostty/config
```

Dotty also keeps machine-specific data outside the repository:

```text
~/.config/dotty/
└── config.toml            # Local configuration, including repository location

~/.local/share/dotty/      # Default XDG data location
├── backups.toml           # Backup metadata
└── ...                    # Backup contents
```

The paths under `~/.config` and `~/.local/share` follow XDG conventions and may differ when the corresponding XDG environment variables are configured. Only the repository's `.dotty/manifest.toml` is intended to travel with the dotfiles across machines.
