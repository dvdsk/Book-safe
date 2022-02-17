I often "forget" the time when reading a great book. In the past there was nothing that could be
done about that. Luckily for my sleep I am reading on a remarkable table running linux. Book safe hides one or more folders from the remarkable ui between a given time period.

No data is ever removed or copied to ensure data integrity in case the tablet unexpectedly shuts down. To hide folders in the gui their content is moved to a different directory. During the move the gui app that runs the remarkable interface is shut down. Though probably unnesesary it is the only way to be sure the remarkable gui does not disrubt the move.

In the case anything goes wrong hidden content can be restored by moving the entire content of `/root/home/locked_books` back to `/home/root/.local/share/xochitl`. Be sure to have a backup of all your documents before you try this, it is still rather new.
