## Temporarily lock files and folders on your reMarkable
I often "forget" the time when reading a great book. In the past there was nothing that could be done about that. In the time of e-readers however, we can. _Book safe_ hides the content of one or more folders from the reMarkable UI within a given time period, and adds a pdf that lists what has been blocked. While folders are blocked, the cloud sync is disabled by default, to prevent deleting and re-uploading of files.

#### Be sure to have a backup of all your documents before you try this!
Even if my code contains zero bugs (we wish) in the future, reMarkable might decide to _"clean up"_ files moved by this application. That would delete any locked files. __Note that the _reMarkable cloud_ is not a backup!__ The _reMarkable cloud_ simply mirrors what is on the device, if it loses your files and it syncs, they will be gone from the cloud too.

#### Usage
On the reMarkable, run the book-safe binary with one of the following subcommands:
```
help         Print this message or the help of the given subcommand(s)
install      Create and enable book-safe system service, locking and unlocking at those times. This command requires additional arguments, call it with --help to see them
list-tz      List supported timezones
run          Lock or unlock right now depending on the time
uninstall    Remove book-safe service and unlock all files. This command requires additional arguments, call it with --help to see them
unlock       Unlock all files
```
The `install` and `run` command _take additional arguments_:
```
    --allow-sync             Do not block sync when locking books, the sync will delete and re-upload books when locking and unlocking!
-e, --end <END>              When to release folders, format: 23:59
-h, --help                   Print help information
-p, --path <PATH>            Path of a folder to be locked (as seen in the ui), pass multiple times to block multiple folders
-s, --start <START>          When to hide folders, format: 23:59
-z, --timezone <TIMEZONE>    Timezone, needed as remarkable resets the device's timezone to UTC on every update
```

Example: set up the book-safe service to lock the folders _Books_ and _hobby_ (which is inside the _Articles_ folder), between 11pm and 8am:
```
book-safe install --start 23:00 --end 8:00 --path Books --path Articles/hobby --timezone Europe/Amsterdam
```

#### Safety
No data is ever removed or copied to ensure data integrity if the tablet unexpectedly shuts down. To hide folders in the GUI, their content is moved to a different directory. The GUI app that runs the reMarkable interface is shut down during the moving of files. This is the only way to be sure the reMarkable GUI does not disrupt the move.

The cloud sync is disabled while files are blocked, unless _Book safe_ is installed with the `--allow-sync` flag. If the cloud sync is not disabled, all blocked files will be deleted from and re-uploaded to the cloud with every block and unblock.

To disable the cloud sync, _Book safe_ blocks network to the reMarkable server by changing the Linux firewall. These changes are lost on reboot. If anything goes wrong, sync can thus be re-enabled by rebooting the device. It is also strongly recommended to disable `auto power-off` in `settings->battery`, because the auto power-off will re-enable the sync while the files are still blocked.

In case anything goes wrong, hidden content can be restored by moving the entire content of `/root/home/locked_books` back to `/home/root/.local/share/xochitl`.

#### Setup 
- Download the latest stable release [binary](https://github.com/dvdsk/Book-safe/releases).
- Move it to anywhere on your reMarkable. I usually place it in `/home/root`.
- _[optional]_ Turn off auto power-off on the reMarkable.
- Run the binary with the `install` subcommand. _Note: each time reMarkable updates, you will need to install Book safe again, or it will either not run or activate at old times._

#### Dev Setup
Requires a _Unix_ OS.

- As always, make a backup.
- Setup [cargo cross](https://github.com/cross-rs/cross).
- _[optional]_ Turn off auto power-off on the reMarkable.
- Use `deploy.sh` to move the binary to the device (set the `SERVER_ADDR` in `deploy.sh` or ensure you have an ssh config entry called `remarkable`).`
- Run book-safe on the device.
