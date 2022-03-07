I often "forget" the time when reading a great book. In the past there was nothing that could be
done about that. In the time of ereaders however, we can. _Book safe_ hides the content of one or more folders from the remarkable ui between a given time period and adds a pdf listing what has been blocked. While folders are blocked the cloud sync is disabled.

#### Usage
Ssh into the remarkable and then binary with a subcommands are:
```
help         Print this message or the help of the given subcommand(s)
install      Create and enable book-safe system service, locking and unlocking at those times
run          Lock or unlock right now depending on the time
uninstall    Remove book-safe service and unlock all files
unlock       Unlock all files
```
The install and run command _require addional arguments_:
```
-s, --start <START>    when to hide folders, format 23:59
-e, --end <END>        when to release folders, format 23:59
-l, --lock <LOCK>      path of a folder to be locked as seen in the ui, 
                       pass multiple times to block multiple folders
```

Example, install a service to lock the folder _Books_ and subfolder hobby which is inside the Articles folder between 11pm and 8am:
```
book-safe install --start 23:00 --end 8:00 --lock Books --lock Articles/hobby
```

#### Safety
No data is ever removed or copied to ensure data integrity in case the tablet unexpectedly shuts down. To hide folders in the gui their content is moved to a different directory. During the move the gui app that runs the remarkable interface is shut down. Though probably unnesesary it is the only way to be sure the remarkable gui does not disrubt the move.

The cloud sync is disabled while files are blocked. If the cloud sync is not disabled all blocked files will be deleted from the cloud. If anything goes wrong the sync can be re-enabled by rebooting the device.

In the case anything goes wrong hidden content can be restored by moving the entire content of `/root/home/locked_books` back to `/home/root/.local/share/xochitl`. You will also need a restart to reset the kernel ip routing table (it is used to block sync while files are blocked). Be sure to have a backup of all your documents before you try this, it is still rather new.

#### Setup
Requires a _unix_ os to set up.

- setup [cargo cross](https://github.com/cross-rs/cross)
- set the `SERVER_ADDR` in `deploy.sh` 
- _[optional]_ change the `SERVER_DIR` to where you want to 'install' book-safe
- run `deploy.sh`
- _[optional]_ set the timezone on the device, that way you do not need to enter the time in UTC. Use:
```bash
timedatectl list-timezones
timedatectl set-timezone <your_time_zone>
```
