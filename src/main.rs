#[macro_use(quickcheck)]
use std::fs::File;
use std::io::prelude::*;
use std::process::Command;
use std::thread;

use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread::sleep;
use std::thread::JoinHandle;

use chrono::prelude::*;
use chrono::Duration;
use clap::{App, Arg};

use std::collections::HashMap;

mod parser;
use parser::parse_crontab;

mod expr;
use expr::*;

struct Args {
    pub crontab_path: String,
    pub edit_flag: bool,
}

#[derive(Clone, Debug)]
pub struct CronEntry {
    minute: CronExpr,
    hour: CronExpr,
    dom: CronExpr,
    month: CronExpr,
    dow: CronExpr,
    startup: bool,
    cmd: String,
}

fn is_leap_year(year: i32) -> bool {
    ((year % 4 == 0) && (year % 100 != 0)) || (year % 400 == 0)
}

fn month_length(date: &DateTime<Local>) -> u32 {
    match date.month() {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => match is_leap_year(date.year()) {
            true => 29,
            false => 28,
        },
        _ => unimplemented!(),
    }
}

fn month_has_day(date: &DateTime<Local>, day: u32) -> bool {
    month_length(date) >= day
}

impl CronEntry {
    pub fn new_startup_task(cmd: &str) -> CronEntry {
        CronEntry {
            minute: CronExpr(CronUnit::Minute, CronInterval::Every),
            hour: CronExpr(CronUnit::Hour, CronInterval::Every),
            dom: CronExpr(CronUnit::DayOfMonth, CronInterval::Every),
            month: CronExpr(CronUnit::Month, CronInterval::Every),
            dow: CronExpr(CronUnit::DayOfWeek, CronInterval::Every),
            startup: true,
            cmd: cmd.to_owned(),
        }
    }

    pub fn next_execution(&self, mut now: DateTime<Local>) -> DateTime<Local> {
        loop {
            if self.minute.is_multiple() && !self.minute.contains(now.minute()) {
                let minute = self.minute.next_from(now.minute());
                if minute < now.minute() {
                    now = now + Duration::hours(1);
                }
                now = now.with_minute(minute).unwrap();
            }

            if self.hour.is_multiple() && !self.hour.contains(now.hour()) {
                let hour = self.hour.next_from(now.hour());
                if hour < now.hour() {
                    now = now.with_hour(hour).unwrap().with_minute(0).unwrap();
                    now = now + Duration::days(1);
                    continue;
                }
                now = now.with_hour(hour).unwrap().with_minute(0).unwrap();
                continue;
            }

            let weekday = now.weekday().num_days_from_sunday();
            if self.dow.is_multiple() && !self.dow.contains(weekday) {
                let delta = (self.dow.next_from(weekday) as i64 - weekday as i64 + 7) % 7;
                now = now + Duration::days(delta);
                now = now.with_hour(0).unwrap().with_minute(0).unwrap();
                continue;
            }

            let next_month = now.month() % 12 + 1;
            if self.dom.is_multiple() && !self.dom.contains(now.day()) {
                let day = self.dom.next_from(now.day());
                if day < now.day() || month_has_day(&now.with_month(next_month).unwrap(), day) {
                    let len = month_length(&now);
                    let days_left_in_month = len - now.day() + 1;
                    now = now + Duration::days(days_left_in_month as i64);
                    now = now
                        .with_day(1)
                        .unwrap()
                        .with_hour(0)
                        .unwrap()
                        .with_minute(0)
                        .unwrap();
                    continue;
                }
                now = now
                    .with_day(day)
                    .unwrap()
                    .with_hour(0)
                    .unwrap()
                    .with_minute(0)
                    .unwrap();
                continue;
            }

            if self.month.is_multiple() && !self.month.contains(now.month()) {
                let month = self.month.next_from(now.month());
                if month < now.month() {
                    let months_to_add = 12 - now.month() + month;
                    let next_month = now.month() + months_to_add;
                    if next_month > 12 {
                        now.with_year(now.year() + 1)
                            .unwrap()
                            .with_month(next_month % 13 + 1)
                            .unwrap()
                            .with_day(1)
                            .unwrap()
                            .with_hour(0)
                            .unwrap()
                            .with_minute(0)
                            .unwrap();
                        continue;
                    }
                    now.with_month(next_month)
                        .unwrap()
                        .with_day(1)
                        .unwrap()
                        .with_hour(0)
                        .unwrap()
                        .with_minute(0)
                        .unwrap();
                    continue;
                }
                now.with_month(month)
                    .unwrap()
                    .with_day(1)
                    .unwrap()
                    .with_hour(0)
                    .unwrap()
                    .with_minute(0)
                    .unwrap();
                continue;
            }
            break;
        }

        return now.with_second(0).unwrap().with_nanosecond(0).unwrap();
    }
}

pub enum Message {
    Quit,
}
pub struct CronJob {
    entry: CronEntry,
    tx: Sender<Message>,
}

impl CronJob {
    pub fn id(&self) -> &String {
        &self.entry.cmd
    }
    pub fn cancel(&self) {
        self.tx.send(Message::Quit).unwrap();
    }
}

pub struct CronScheduler {
    cron_path: String,
    jobs: HashMap<String, CronJob>,
}

impl CronScheduler {
    pub fn new(cron_path: String) -> CronScheduler {
        CronScheduler {
            cron_path,
            jobs: HashMap::new(),
        }
    }

    pub fn read_crontab(&mut self) -> std::io::Result<()> {
        let mut crontab_file = File::open(&self.cron_path)?;
        let mut crontab_string = String::new();
        crontab_file.read_to_string(&mut crontab_string)?;

        let crontab = parse_crontab(&crontab_string);
        for entry in crontab {
            self.start_job(entry);
            sleep(Duration::milliseconds(100).to_std().unwrap());
        }
        return Ok(());
    }

    pub fn start_job(&mut self, entry: CronEntry) {
        let (tx, rx) = channel();
        spawn_job(&entry, rx);
        let cronjob = CronJob { entry, tx };
        self.jobs.insert(cronjob.id().clone(), cronjob);
    }

    pub fn clear(&mut self) {
        for job in self.jobs.values() {
            job.cancel();
        }
        self.jobs.clear();
    }
}

fn main() {
    let args = gen_args();

    if args.edit_flag {
        if std::env::var("EDITOR").is_err() {
            println!("Error reading $EDITOR, make sure it is set correctly and try again");
            std::process::exit(1);
        }
        let cmd = vec!["$EDITOR", &args.crontab_path].join(" ");
        Command::new("/bin/sh")
            .arg("-c")
            .arg(&cmd)
            .output()
            .expect("failed to edit the crontab");
        std::process::exit(1);
    }

    start_cronjobs(args.crontab_path);
}

fn gen_args() -> Args {
    let matches = App::new("Crust")
        .version("0.3.1")
        .author("Karl David Hedgren. <david@davebay.net>")
        .about("rust + cron = crust!\n A cron manager in rust!")
        .arg(
            Arg::with_name("crontab")
                .short("c")
                .long("crontab")
                .help("Use crontab file at PATH")
                .value_name("PATH")
                .takes_value(true)
                .default_value("$XDG_CONFIG_HOME/crontab"),
        )
        .arg(
            Arg::with_name("edit")
                .short("e")
                .long("edit")
                .help("Open the crontab in your editor"),
        )
        .get_matches();

    let config_path = matches.value_of("crontab").unwrap();
    let home = std::env::var("HOME").unwrap_or(String::from("/"));
    let xdg_config_path =
        std::env::var("XDG_CONFIG_HOME").unwrap_or(vec![home, String::from("/.config")].join(""));
    let crontab_path = config_path.replace("$XDG_CONFIG_HOME", &xdg_config_path);

    Args {
        crontab_path: String::from(crontab_path),
        edit_flag: matches.is_present("edit"),
    }
}

fn start_cronjobs(cron_path: String) {
    let mut scheduler = CronScheduler::new(cron_path.clone());
    match scheduler.read_crontab() {
        Err(_) => panic!("Failed to read crontab!"),
        _ => {}
    };
    loop {}
}

fn spawn_job(entry: &CronEntry, rx: Receiver<Message>) -> JoinHandle<()> {
    let entry = (*entry).clone();
    thread::spawn(move || loop {
        if entry.startup {
            Command::new("/bin/sh")
                .arg("-c")
                .arg(&entry.cmd)
                .spawn()
                .expect("failed to execute process");
            break;
        }
        let future = entry.next_execution(Local::now() + Duration::minutes(1));
        println!("Scheduling: `{}` for {}", entry.cmd, future);
        let t = future - Local::now();
        sleep(t.to_std().unwrap());

        // Check if this is a stale thread
        if let Ok(msg) = rx.try_recv() {
            match msg {
                Message::Quit => return,
            }
        }

        Command::new("/bin/sh")
            .arg("-c")
            .arg(&entry.cmd)
            .spawn()
            .expect("failed to execute process");
    })
}
