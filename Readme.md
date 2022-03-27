## Be sure to have a backup of all your documents before you try this, it is still rather new.
Even if my code contains zero bugs (we wish) in the future, remarkable might decide to _"clean up"_ files moved by this application losing their content. __Note the _reMarkable cloud_ is not a backup!__ The _reMarkable cloud_ simply mirrors what is on the device, if it loses your files and it syncs they will be gone from the cloud too.

I often "forget" the time when reading a great book. In the past there was nothing that could be
done about that. In the time of ereaders however, we can. _Book safe_ hides the content of one or more folders from the reMarkable ui within a given time period and adds a pdf that lists what has been blocked. While folders are blocked the cloud sync is disabled.

#### Usage
On the the remarkable run book-safe with one of the subcommands:
```
help         Print this message or the help of the given subcommand(s)
install      Create and enable book-safe system service, locking and unlocking at those times
run          Lock or unlock right now depending on the time
uninstall    Remove book-safe service and unlock all files
unlock       Unlock all files
```
The install and run command _require additional arguments_:
```
-s, --start <START>    when to hide folders, format 23:59
-e, --end <END>        when to release folders, format 23:59
-l, --lock <LOCK>      path of a folder to be locked as seen in the ui, 
                       pass multiple times to block multiple folders
```

Example, set-up the book-safe service to lock the folder _Books_ and subfolder _hobby_ which is inside the _Articles_ folder, between 11pm and 8am:
```
book-safe install --start 23:00 --end 8:00 --lock Books --lock Articles/hobby
```

#### Safety
No data is ever removed or copied to ensure data integrity in case the tablet unexpectedly shuts down. To hide folders in the gui their content is moved to a different directory. During the move the gui app that runs the remarkable interface is shut down. This is the only way to be sure the remarkable gui does not disrubt the move.

The cloud sync is disabled while files are blocked. If the cloud sync is not disabled all blocked files will be deleted from the cloud. _Book safe_ blocks network to the remarkable server by changing the linux firewall. These changes are lost on reboot. If anything goes wrong sync can thus be re-enabled by rebooting the device. It is also strongely advised to disable `auto power-off` in `settings->battery`.

In case anything goes wrong hidden content can be restored by moving the entire content of `/root/home/locked_books` back to `/home/root/.local/share/xochitl`. If you moved the _Book safe_ binary the `locked_books` directory will be next to it.

#### Setup
Requires a _unix_ os to set up.

- as always make a backup
- setup [cargo cross](https://github.com/cross-rs/cross)
- set the `SERVER_ADDR` in `deploy.sh` 
- _[optional]_ Turn off auto poweroff on the remarkable
- _[optional]_ change the `SERVER_DIR` to where you want to 'install' book-safe
- run `deploy.sh`
- _[optional]_ set the timezone on the device, that way you do not need to enter the time in UTC. _note you will need to do this again after the next remarkable update._ Use:
```bash
timedatectl list-timezones
timedatectl set-timezone <your_time_zone>
```
- run booksafe with `install` subcommand, _note: each time remarkable updates you will need to install booksafe again or it will either not run or activate at old times._
