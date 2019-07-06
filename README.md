# crust
rust + cron = crust!

A cron manager in rust, since the cool new thing is remaking old linux utilities in rust!

WARNING!
Has not yet been extensively tested!

# Installing
Simply clone the repository and run `cargo build --release` in the project directory.
The executable will then be built to `./target/release/crust`.

# Using
Currently does not currently (and probably never will) run as a deamon on its own.
Either run it using a service manager such as systemd or put it in a user startup script.
The default crontab path is $HOME/.config/crontab

# Todo
* Respect xdg config directory
* Add support for the non-standard predefined scheduling commands, see [this link](https://en.wikipedia.org/wiki/Cron#Nonstandard_predefined_scheduling_definitions)
* Add tests, preferably using quickcheck
